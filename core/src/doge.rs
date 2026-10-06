//! Dogecoin and DogecoinVM: amounts, fees and payments in the legacy
//! (pre-SegWit) format both chains use, as DogecoinVM's web wallet makes
//! them in `chain.js` (MetalBlockchain/dogecoin-vm, `cmd/dogevm/web`). It
//! must agree byte for byte: `tests/doge_vectors.rs` checks it against the
//! vectors that wallet generated and btcd's script engine verified.
//!
//! The wallet's own address on both chains is P2PKH of its compressed key
//! (`D…`). Legacy signatures don't commit to the amounts they spend, so a
//! server lying about a coin's value could turn the difference into fee:
//! each value is taken from the transaction that created the coin, checked
//! against its id, before anything is signed.

use crate::address::{Destination, Kind, push_data};
use crate::amount::{format_units, parse_units};
use crate::keys::Key;
use crate::payment::{
    Chain, Plan, Request, Signed, Utxo, checked_signed, payment_outputs, signature, spendable,
    verified_input,
};
use crate::tx::{LEGACY_SEQUENCE, LEGACY_TX_VERSION, Tx, TxOut};
use crate::{Error, Result, invalid};
use std::collections::HashMap;

pub const KOINU_PER_DOGE: u64 = 100_000_000;
/// Dogecoin's MAX_MONEY: the most a single output may hold.
pub const MAX_MONEY: u64 = 10_000_000_000 * KOINU_PER_DOGE;
/// Dogecoin's recommended wallet fee, 0.01 DOGE/kB, in koinu per byte.
pub const FEE_PER_BYTE: u64 = 1_000;
/// DogecoinVM's relay minimum, a tenth of that. Its blocks have room to
/// spare, so the minimum always makes the next one.
pub const VM_FEE_PER_BYTE: u64 = 100;
/// Dogecoin's soft dust limit, 0.01 DOGE: a payment below it costs as much
/// again in fee, and change below it goes to the fee.
pub const SOFT_DUST: u64 = KOINU_PER_DOGE / 100;
/// Its hard dust limit, 0.001 DOGE: outputs below it aren't relayed. The
/// smallest payment.
pub const HARD_DUST: u64 = SOFT_DUST / 10;
/// No payment should cost more than this in fees, 5 DOGE; a larger figure
/// means something is wrong, so it is refused.
pub const MAX_FEE: u64 = 5 * KOINU_PER_DOGE;

/// Parses a DOGE amount like "12.5", with up to 8 decimals, into koinu.
pub fn parse_doge(s: &str) -> Result<u64> {
    parse_units(s, 11, MAX_MONEY, "12.5", "more than 10 billion DOGE")
}

/// Formats koinu as DOGE, without trailing zeros: 1250000000 is "12.5".
pub fn format_doge(koinu: u64) -> String {
    format_units(koinu)
}

/// The fee rate a payment on `chain` pays, in koinu per byte.
pub fn fee_per_byte(chain: Chain) -> u64 {
    if chain.is_vm() {
        VM_FEE_PER_BYTE
    } else {
        FEE_PER_BYTE
    }
}

/// The size in bytes a payment's fee is reckoned on, as chain.js reckons it:
/// its signed P2PKH inputs, its outputs and change, and an OP_RETURN's data.
fn payment_size(inputs: usize, outputs: usize, data_len: usize) -> u64 {
    let data = if data_len > 0 { data_len + 3 } else { 0 };
    (10 + 149 * inputs + 34 * (outputs + 1) + data) as u64
}

/// What a payment of `amount` pays on top of its size: one below the soft
/// dust limit costs that limit again.
fn dust_fee(amount: u64) -> u64 {
    if amount < SOFT_DUST { SOFT_DUST } else { 0 }
}

fn check_from(from: &Destination) -> Result<()> {
    if from.kind() != Kind::P2pkh {
        return invalid("the wallet spends only its own Dogecoin coins (P2PKH)");
    }
    Ok(())
}

/// The coins of `from` the server listed, each checked against the
/// transaction that created it, largest first by the value the server
/// claims, as chain.js orders them.
fn verified_coins<'a>(
    from_script: &[u8],
    utxos: &'a [Utxo],
) -> impl Iterator<Item = &'a Utxo> + use<'a> {
    let mut coins: Vec<(u64, &Utxo)> = spendable(utxos, from_script)
        .map(|u| (u.value.parse().unwrap_or(0), u))
        .collect();
    coins.sort_by_key(|&(value, _)| std::cmp::Reverse(value));
    coins.into_iter().map(|(_, u)| u)
}

/// Plans a payment on Dogecoin or DogecoinVM from the coins of `from` (the
/// wallet's P2PKH address), as [`crate::payment::plan_payment`] does on
/// Bitcoin: coins largest first until they cover the amount and the fee,
/// change below the soft dust limit to the fee. The fee is the chain's
/// fixed rate on chain.js's size estimate.
pub(crate) fn plan_payment(
    from: &Destination,
    utxos: &[Utxo],
    raw_txs: &HashMap<String, String>,
    req: &Request,
) -> Result<Plan> {
    check_from(from)?;
    if req.amount < HARD_DUST {
        return invalid(format!(
            "the smallest payment is {} DOGE",
            format_doge(HARD_DUST)
        ));
    }
    if req.amount > MAX_MONEY {
        return invalid("more than 10 billion DOGE");
    }
    let from_script = from.pk_script();
    let mut outputs = payment_outputs(req)?;
    let data_len = req.data.as_ref().map_or(0, Vec::len);
    let per_byte = fee_per_byte(req.chain);

    let mut inputs = Vec::new();
    let (mut total, mut fee) = (0u64, 0u64);
    for u in verified_coins(&from_script, utxos) {
        let mut input = verified_input(u, raw_txs, &from_script)?;
        input.sequence = LEGACY_SEQUENCE;
        total = total
            .checked_add(input.value)
            .ok_or_else(|| Error::Untrusted("coin values overflow".into()))?;
        inputs.push(input);
        fee = payment_size(inputs.len(), outputs.len(), data_len) * per_byte + dust_fee(req.amount);
        if total >= req.amount + fee {
            break;
        }
    }
    if total < req.amount + fee {
        return invalid(format!(
            "not enough confirmed DOGE on {}: have {}, need {} including the fee",
            req.chain.name(),
            format_doge(total),
            format_doge(req.amount + fee)
        ));
    }
    let change = total - req.amount - fee;
    if change >= SOFT_DUST {
        outputs.push(TxOut {
            value: change,
            script: from_script,
        });
    } else {
        fee += change;
    }
    if fee > MAX_FEE {
        return invalid(format!(
            "the fee would be {} DOGE; refusing to sign",
            format_doge(fee)
        ));
    }
    let tx = Tx {
        version: LEGACY_TX_VERSION,
        inputs,
        outputs,
    };
    let unsigned = tx.serialize();
    Ok(Plan {
        chain: req.chain,
        fee,
        total_in: total,
        tx,
        from: from.clone(),
        unsigned,
    })
}

/// The most a payment can send (with `req.data`) on Dogecoin or DogecoinVM:
/// every confirmed coin of `from`, checked, less the fee, so nothing is left
/// for change. Sized as [`plan_payment`] sizes it, so planning this amount
/// spends every coin. `req.amount` is ignored.
pub(crate) fn max_payment(
    from: &Destination,
    utxos: &[Utxo],
    raw_txs: &HashMap<String, String>,
    req: &Request,
) -> Result<u64> {
    check_from(from)?;
    let from_script = from.pk_script();
    let (mut total, mut count) = (0u64, 0usize);
    for u in spendable(utxos, &from_script) {
        let input = verified_input(u, raw_txs, &from_script)?;
        total = total
            .checked_add(input.value)
            .ok_or_else(|| Error::Untrusted("coin values overflow".into()))?;
        count += 1;
    }
    let data_len = req.data.as_ref().map_or(0, Vec::len);
    let fee = payment_size(count, payment_outputs(req)?.len(), data_len) * fee_per_byte(req.chain);
    if fee > MAX_FEE {
        return invalid(format!(
            "the fee would be {} DOGE; refusing to sign",
            format_doge(fee)
        ));
    }
    // An amount below the soft dust limit would pay that much again, so the
    // most is never below it.
    match total.checked_sub(fee) {
        Some(amount) if amount >= SOFT_DUST => Ok(amount.min(MAX_MONEY)),
        _ => invalid(format!(
            "not enough confirmed DOGE on {} to cover the fee",
            req.chain.name()
        )),
    }
}

/// Signs a planned Dogecoin or DogecoinVM payment: each input carries its
/// legacy signature and the compressed key. The signed transaction is read
/// back and must spend and pay exactly what was reviewed.
pub(crate) fn sign_plan(key: &Key, plan: &Plan) -> Result<Signed> {
    let from = key.p2pkh_destination();
    if from != plan.from {
        return invalid("this key does not own the plan's coins");
    }
    let from_script = from.pk_script();
    let public = key.public_key();
    let mut tx = plan.tx.clone();
    for i in 0..tx.inputs.len() {
        let der = signature(key, &tx.legacy_sighash(i, &from_script))?;
        tx.inputs[i].script_sig = [push_data(&der)?, push_data(&public)?].concat();
    }
    checked_signed(&tx, plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts() {
        assert_eq!(parse_doge("12.5").unwrap(), 1_250_000_000);
        assert_eq!(parse_doge("10000000000").unwrap(), MAX_MONEY);
        assert!(parse_doge("10000000000.00000001").is_err());
        assert!(parse_doge("1.123456789").is_err());
        assert!(parse_doge("").is_err());
        assert_eq!(format_doge(1_250_000_000), "12.5");
        assert_eq!(format_doge(HARD_DUST), "0.001");
        assert_eq!(format_doge(MAX_MONEY), "10000000000");
    }

    #[test]
    fn sizes_as_chain_js() {
        // One input, the payment and change: 10 + 149 + 68.
        assert_eq!(payment_size(1, 1, 0), 227);
        // A DVMO tag adds its output and its 25 bytes of data, plus 3.
        assert_eq!(payment_size(1, 2, 25), 289);
        assert_eq!(dust_fee(SOFT_DUST - 1), SOFT_DUST);
        assert_eq!(dust_fee(SOFT_DUST), 0);
    }
}
