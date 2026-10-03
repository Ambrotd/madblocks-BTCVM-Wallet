//! Planning and signing payments from the wallet's P2WPKH coins, as the web
//! wallet's chain.js does: the same choice of coins, the same fees and the
//! same bytes.

use crate::address::{Destination, Kind, Network, push_data};
use crate::amount::{MAX_MONEY, format_btc};
use crate::bridge::peg_out_destination;
use crate::keys::Key;
use crate::tx::{SEQUENCE, TX_VERSION, Tx, TxIn, TxOut, parse_tx, vsize};
use crate::{Error, Result, invalid, untrusted};
use k256::ecdsa::{Signature, signature::hazmat::PrehashSigner};
use std::collections::HashMap;

/// Bitcoin's fee rate in sat/vB when the bridge gives none.
pub const BTC_FEE_RATE: u64 = 5;
/// BTCVM's relay minimum, 1 sat/kvB, and never less than a satoshi: its
/// blocks have room to spare, so the minimum makes the next one.
pub const VM_FEE_PER_KVB: u64 = 1;
/// The smallest output on Bitcoin, Core's dust threshold for the largest
/// standard output. Change below it goes to the fee.
pub const DUST: u64 = 546;
/// On BTCVM a single satoshi is never dust.
pub const VM_DUST: u64 = 1;
/// No payment should cost more than this in fees, in satoshis; a larger
/// figure means something is wrong, so it is refused.
pub const MAX_FEE: u64 = 250_000;
/// Nor pay more than this fee rate, in sat/vB, whatever the bridge suggests.
pub const MAX_FEE_RATE: u64 = 1_000;

/// Which chain a payment is on. They share formats, not coins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    Bitcoin,
    Btcvm,
}

impl Chain {
    pub fn name(self) -> &'static str {
        match self {
            Chain::Bitcoin => "Bitcoin",
            Chain::Btcvm => "BTCVM",
        }
    }

    fn dust(self) -> u64 {
        match self {
            Chain::Bitcoin => DUST,
            Chain::Btcvm => VM_DUST,
        }
    }
}

/// An unspent output as the server lists it. Only its txid and vout are
/// taken on trust: the value signed for comes from the transaction that
/// created it, checked against its id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    /// Satoshis, as a string.
    pub value: String,
    /// The output script, hex.
    pub script: String,
    pub confirmations: i64,
}

/// What to pay.
#[derive(Debug, Clone)]
pub struct Request {
    pub chain: Chain,
    pub to: Destination,
    pub amount: u64,
    /// An OP_RETURN to carry, such as a BVMO withdrawal tag.
    pub data: Option<Vec<u8>>,
    /// sat/vB on Bitcoin. BTCVM pays its relay minimum and ignores it.
    pub fee_rate: u64,
}

/// A payment ready for review: the coins it spends, with their values
/// checked, what it pays, and its fee. Signing it signs exactly this.
#[derive(Debug, Clone)]
pub struct Plan {
    pub chain: Chain,
    pub fee: u64,
    pub total_in: u64,
    tx: Tx,
    from: Destination,
    unsigned: Vec<u8>,
}

impl Plan {
    /// The outputs, in order: the payment first, then any OP_RETURN, then
    /// any change.
    pub fn outputs(&self) -> &[TxOut] {
        &self.tx.outputs
    }

    /// The coins it spends, as `txid:vout`.
    pub fn spends(&self) -> Vec<String> {
        self.tx
            .inputs
            .iter()
            .map(|i| format!("{}:{}", hex::encode(i.txid), i.vout))
            .collect()
    }

    /// The unsigned transaction, hex.
    pub fn unsigned_hex(&self) -> String {
        hex::encode(&self.unsigned)
    }

    /// The outputs described for the review screen.
    pub fn describe(&self, net: &Network) -> Vec<OutputView> {
        describe_outputs(&self.tx.outputs, &self.from, net)
    }
}

/// A signed payment, ready to broadcast.
#[derive(Debug, Clone)]
pub struct Signed {
    pub hex: String,
    pub txid: String,
    pub fee: u64,
}

/// Plans a payment from the coins of `from` (the wallet's P2WPKH address)
/// that the server listed in `utxos`. `raw_txs` maps each coin's txid to the
/// hex of the transaction that created it, which is checked against the id
/// before its value is used. Coins are taken largest first until they cover
/// the amount and the fee; change below the dust limit goes to the fee.
pub fn plan_payment(
    from: &Destination,
    utxos: &[Utxo],
    raw_txs: &HashMap<String, String>,
    req: &Request,
) -> Result<Plan> {
    if from.kind() != Kind::P2wpkh {
        return invalid("the wallet spends only its own native SegWit coins");
    }
    let dust = req.chain.dust();
    if req.amount < dust {
        return invalid(format!("the smallest payment is {} BTC", format_btc(dust)));
    }
    if req.amount > MAX_MONEY {
        return invalid("more than 21 million BTC");
    }
    if req.chain == Chain::Bitcoin && !(1..=MAX_FEE_RATE).contains(&req.fee_rate) {
        return invalid(format!(
            "a fee rate of {} sat/vB looks wrong; refusing",
            req.fee_rate
        ));
    }
    let from_script = from.pk_script();
    let mut spendable: Vec<(u64, &Utxo)> = spendable(utxos, &from_script)
        .map(|u| (u.value.parse().unwrap_or(0), u))
        .collect();
    // Largest first, by the value the server claims, as chain.js does.
    spendable.sort_by_key(|&(value, _)| std::cmp::Reverse(value));

    let mut outputs = payment_outputs(req)?;
    let sized = with_change(&outputs, &from_script);

    let mut inputs: Vec<TxIn> = Vec::new();
    let (mut total, mut fee) = (0u64, 0u64);
    for (_, u) in spendable {
        let input = verified_input(u, raw_txs, &from_script)?;
        total = total
            .checked_add(input.value)
            .ok_or_else(|| Error::Untrusted("coin values overflow".into()))?;
        inputs.push(input);
        fee = fee_for(req, vsize(inputs.len(), &sized));
        if total >= req.amount + fee {
            break;
        }
    }
    if total < req.amount + fee {
        return invalid(format!(
            "not enough confirmed BTC on {}: have {}, need {} including the fee",
            req.chain.name(),
            format_btc(total),
            format_btc(req.amount + fee)
        ));
    }
    let change = total - req.amount - fee;
    if change >= dust {
        outputs.push(TxOut {
            value: change,
            script: from_script,
        });
    } else {
        fee += change;
    }
    if fee > MAX_FEE {
        return invalid(format!(
            "the fee would be {} BTC; refusing to sign",
            format_btc(fee)
        ));
    }
    let tx = Tx {
        version: TX_VERSION,
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

/// The most a payment can send to `req.to` (with `req.data`) on
/// `req.chain`: every confirmed coin of `from`, checked as `plan_payment`
/// checks them, less the fee, so nothing is left for change. It is sized as
/// `plan_payment` sizes a payment, so planning this amount spends every coin.
/// `req.amount` is ignored.
pub fn max_payment(
    from: &Destination,
    utxos: &[Utxo],
    raw_txs: &HashMap<String, String>,
    req: &Request,
) -> Result<u64> {
    if from.kind() != Kind::P2wpkh {
        return invalid("the wallet spends only its own native SegWit coins");
    }
    if req.chain == Chain::Bitcoin && !(1..=MAX_FEE_RATE).contains(&req.fee_rate) {
        return invalid(format!(
            "a fee rate of {} sat/vB looks wrong; refusing",
            req.fee_rate
        ));
    }
    let from_script = from.pk_script();
    let (mut total, mut count) = (0u64, 0usize);
    for u in spendable(utxos, &from_script) {
        let input = verified_input(u, raw_txs, &from_script)?;
        total = total
            .checked_add(input.value)
            .ok_or_else(|| Error::Untrusted("coin values overflow".into()))?;
        count += 1;
    }
    let fee = fee_for(
        req,
        vsize(count, &with_change(&payment_outputs(req)?, &from_script)),
    );
    if fee > MAX_FEE {
        return invalid(format!(
            "the fee would be {} BTC; refusing to sign",
            format_btc(fee)
        ));
    }
    match total.checked_sub(fee) {
        Some(amount) if amount >= req.chain.dust() => Ok(amount.min(MAX_MONEY)),
        _ => invalid(format!(
            "not enough confirmed BTC on {} to cover the fee",
            req.chain.name()
        )),
    }
}

/// The confirmed coins paying `from_script`, each once, in the server's order.
fn spendable<'a>(utxos: &'a [Utxo], from_script: &[u8]) -> impl Iterator<Item = &'a Utxo> {
    let from_hex = hex::encode(from_script);
    let mut seen = std::collections::HashSet::new();
    utxos.iter().filter(move |u| {
        u.confirmations > 0 && u.script == from_hex && seen.insert((u.txid.clone(), u.vout))
    })
}

/// A payment's own outputs: what it pays, then any OP_RETURN.
fn payment_outputs(req: &Request) -> Result<Vec<TxOut>> {
    let mut outputs = vec![TxOut {
        value: req.amount,
        script: req.to.pk_script(),
    }];
    if let Some(data) = &req.data {
        outputs.push(TxOut {
            value: 0,
            script: [&[0x6a][..], &push_data(data)?].concat(),
        });
    }
    Ok(outputs)
}

/// The outputs a payment is sized for: its own, and change, which it
/// usually has.
fn with_change(outputs: &[TxOut], from_script: &[u8]) -> Vec<TxOut> {
    outputs
        .iter()
        .cloned()
        .chain([TxOut {
            value: 0,
            script: from_script.to_vec(),
        }])
        .collect()
}

fn fee_for(req: &Request, vsize: u64) -> u64 {
    match req.chain {
        Chain::Bitcoin => vsize * req.fee_rate,
        Chain::Btcvm => vm_fee(vsize),
    }
}

/// What a BTCVM transaction of `vsize` vbytes pays: the node's relay
/// minimum, computed as the node does (a satoshi under 1,000 vB).
pub fn vm_fee(vsize: u64) -> u64 {
    match vsize * VM_FEE_PER_KVB / 1000 {
        0 => VM_FEE_PER_KVB,
        fee => fee,
    }
}

/// Checks a coin the server listed against the transaction that created it
/// (its bytes must hash to the coin's txid) and takes the value from those
/// bytes. A SegWit signature commits to the value it spends, so a wrong one
/// would only make the payment invalid; checking first gives a clear error,
/// and confirms the output is the wallet's.
fn verified_input(u: &Utxo, raw_txs: &HashMap<String, String>, from_script: &[u8]) -> Result<TxIn> {
    let raw = raw_txs
        .get(&u.txid)
        .ok_or_else(|| Error::Invalid(format!("no transaction {}", u.txid)))?;
    let raw = hex::decode(raw)
        .map_err(|_| Error::Untrusted(format!("transaction {} is not hex", u.txid)))?;
    let parsed =
        parse_tx(&raw).map_err(|e| Error::Untrusted(format!("transaction {}: {e}", u.txid)))?;
    if parsed.txid() != u.txid {
        return untrusted(format!(
            "the server sent the wrong transaction for {}",
            u.txid
        ));
    }
    let Some(out) = parsed.outputs.get(u.vout as usize) else {
        return untrusted(format!("transaction {} has no output {}", u.txid, u.vout));
    };
    if out.script != from_script {
        return untrusted(format!("output {}:{} is not yours", u.txid, u.vout));
    }
    let mut txid = [0u8; 32];
    hex::decode_to_slice(&u.txid, &mut txid)
        .map_err(|_| Error::Untrusted(format!("bad txid {}", u.txid)))?;
    Ok(TxIn {
        txid,
        vout: u.vout,
        value: out.value,
        sequence: SEQUENCE,
        witness: Vec::new(),
    })
}

/// Signs a plan with the key whose coins it spends, then reads the signed
/// transaction back and checks it spends and pays exactly what was reviewed.
pub fn sign_plan(key: &Key, plan: &Plan) -> Result<Signed> {
    let from = key.destination();
    if from != plan.from {
        return invalid("this key does not own the plan's coins");
    }
    let mut tx = plan.tx.clone();
    let signing = key.signing_key();
    let public = key.public_key();
    for i in 0..tx.inputs.len() {
        let hash = tx.sighash(i, from.program());
        let sig: Signature = signing
            .sign_prehash(&hash)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let sig = sig.normalize_s().unwrap_or(sig);
        let mut der = sig.to_der().as_bytes().to_vec();
        der.push(1); // SIGHASH_ALL
        tx.inputs[i].witness = vec![der, public.to_vec()];
    }
    let raw = tx.serialize();
    let signed = parse_tx(&raw)?;
    let reviewed = parse_tx(&plan.unsigned)?;
    if signed.inputs != reviewed.inputs || signed.outputs != reviewed.outputs {
        return invalid("the signed transaction differs from the one reviewed; nothing was sent");
    }
    Ok(Signed {
        txid: signed.txid(),
        hex: hex::encode(&raw),
        fee: plan.fee,
    })
}

/// Plans and signs in one step.
pub fn build_payment(
    key: &Key,
    utxos: &[Utxo],
    raw_txs: &HashMap<String, String>,
    req: &Request,
) -> Result<Signed> {
    let plan = plan_payment(&key.destination(), utxos, raw_txs, req)?;
    sign_plan(key, &plan)
}

/// An output, described for a person.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputView {
    pub value: u64,
    /// The output script, hex.
    pub script: String,
    /// The address paid, for a standard script.
    pub address: Option<String>,
    /// An OP_RETURN's data, hex.
    pub data: Option<String>,
    /// For a BVMO tag: the Bitcoin address the bridge is asked to pay.
    pub withdrawal_to: Option<String>,
    /// Pays back to the wallet.
    pub change: bool,
}

/// Describes outputs for review: whom each pays, which is change back to
/// `own`, and where a BVMO tag asks the bridge to send a withdrawal.
pub fn describe_outputs(outputs: &[TxOut], own: &Destination, net: &Network) -> Vec<OutputView> {
    outputs
        .iter()
        .map(|o| {
            let dest = Destination::from_script(&o.script);
            let data = op_return_data(&o.script);
            OutputView {
                value: o.value,
                script: hex::encode(&o.script),
                change: dest.as_ref() == Some(own),
                address: dest.map(|d| d.address(net)),
                withdrawal_to: data.and_then(peg_out_destination).map(|d| d.address(net)),
                data: data.map(hex::encode),
            }
        })
        .collect()
}

/// The data of an OP_RETURN carrying a single push, or None.
fn op_return_data(script: &[u8]) -> Option<&[u8]> {
    match script {
        [0x6a, n, data @ ..] if usize::from(*n) < 0x4c && data.len() == usize::from(*n) => {
            Some(data)
        }
        [0x6a, 0x4c, n, data @ ..] if data.len() == usize::from(*n) => Some(data),
        _ => None,
    }
}
