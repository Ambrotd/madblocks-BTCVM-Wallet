//! The three things the wallet does, with the bridge's rules enforced: send
//! on either chain, move BTC to BTCVM (a deposit) and back (a withdrawal).
//! Each returns a plan for review; nothing is signed here.

use crate::address::{Destination, decode_address};
use crate::amount::format_btc;
use crate::bridge::{VerifiedBridge, peg_out_data};
use crate::payment::{Chain, Plan, Request, Utxo, plan_payment};
use crate::{Result, invalid, untrusted};
use std::collections::HashMap;

/// Coins to spend: the server's list for the wallet's address on one chain,
/// and the transactions that created them, hex by txid.
#[derive(Debug, Clone, Copy)]
pub struct Coins<'a> {
    pub utxos: &'a [Utxo],
    pub raw_txs: &'a HashMap<String, String>,
}

/// Why deposits and withdrawals stop while the bridge reports another signer
/// set than the pinned one (see [`crate::bridge::SignerChange`]).
pub const PAUSED_BY_SIGNER_CHANGE: &str = "the bridge's signers have changed, so moving coins between \
     Bitcoin and BTCVM is paused until the wallet is updated with the new set; sends still work";

/// A payment on `chain` to an address the user typed or pasted. Paying the
/// peg directly is refused: without a tag the bridge can't tell whose coins
/// they are, and they would sit there unclaimed.
pub fn plan_send(
    bridge: &VerifiedBridge,
    chain: Chain,
    from: &Destination,
    coins: Coins,
    to: &str,
    amount: u64,
) -> Result<Plan> {
    let to = decode_address(to, &bridge.net)?;
    if bridge.is_peg(&to) {
        return invalid(match chain {
            Chain::Bitcoin => {
                "that is the bridge's own address; use Move to BTCVM, which pays your personal deposit address"
            }
            Chain::Btcvm => {
                "that is the bridge's reserve; use Withdraw to Bitcoin, which tags the payment with your Bitcoin address"
            }
        });
    }
    plan_payment(
        from,
        coins.utxos,
        coins.raw_txs,
        &Request {
            chain,
            to,
            amount,
            data: None,
            fee_rate: bridge.btc_fee_rate,
        },
    )
}

/// A deposit: pays `from`'s personal deposit address on Bitcoin, and the
/// bridge credits the same address on BTCVM once the deposit has enough
/// confirmations. The address is derived here from the pinned signers;
/// `told` is the one the bridge gave (`POST /api/deposit-address`, which
/// also registers it), and they must match.
pub fn plan_deposit(
    bridge: &VerifiedBridge,
    from: &Destination,
    coins: Coins,
    amount: u64,
    told: &str,
) -> Result<Plan> {
    if bridge.signer_change.is_some() {
        return untrusted(PAUSED_BY_SIGNER_CHANGE);
    }
    if amount < bridge.min_deposit {
        return invalid(format!(
            "the smallest deposit is {} BTC; a smaller one is not credited",
            format_btc(bridge.min_deposit)
        ));
    }
    if bridge.max_deposit > 0 && amount > bridge.max_deposit {
        return invalid(format!(
            "a deposit can be at most {} BTC for now; a larger one is held for a refund",
            format_btc(bridge.max_deposit)
        ));
    }
    let deposit = bridge.signers.deposit_destination(from)?;
    if decode_address(told, &bridge.net).ok().as_ref() != Some(&deposit) {
        return untrusted(
            "the bridge gave a deposit address that doesn't follow from the peg's signers, so nothing was sent",
        );
    }
    plan_payment(
        from,
        coins.utxos,
        coins.raw_txs,
        &Request {
            chain: Chain::Bitcoin,
            to: deposit,
            amount,
            data: None,
            fee_rate: bridge.btc_fee_rate,
        },
    )
}

/// A withdrawal: pays the reserve on BTCVM with a BVMO tag naming `to` on
/// Bitcoin. Once the payment is final, the bridge pays the amount there,
/// less Bitcoin's network fee.
pub fn plan_withdrawal(
    bridge: &VerifiedBridge,
    from: &Destination,
    coins: Coins,
    amount: u64,
    to: &str,
) -> Result<Plan> {
    if bridge.signer_change.is_some() {
        return untrusted(PAUSED_BY_SIGNER_CHANGE);
    }
    if amount < bridge.min_peg_out {
        return invalid(format!(
            "the smallest withdrawal is {} BTC; a smaller one is not paid",
            format_btc(bridge.min_peg_out)
        ));
    }
    let dest = decode_address(to, &bridge.net)?;
    if bridge.is_peg(&dest) {
        return invalid("a withdrawal can't pay the bridge's own address");
    }
    plan_payment(
        from,
        coins.utxos,
        coins.raw_txs,
        &Request {
            chain: Chain::Btcvm,
            to: bridge.peg.clone(),
            amount,
            data: Some(peg_out_data(&dest)),
            fee_rate: bridge.btc_fee_rate,
        },
    )
}
