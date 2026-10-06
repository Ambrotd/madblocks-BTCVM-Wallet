//! The three things the wallet does with each coin, with its bridge's rules
//! enforced: send on either of the coin's chains, move it to its VM on Metal
//! (a deposit) and back (a withdrawal). Each returns a plan for review;
//! nothing is signed here.

use crate::address::{Destination, decode_address};
use crate::bridge::{VerifiedBridge, peg_out_data_for};
use crate::payment::{Chain, Coin, Plan, Request, Utxo, plan_payment};
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

/// [`PAUSED_BY_SIGNER_CHANGE`], for `coin`'s bridge.
pub fn paused_by_signer_change(coin: Coin) -> String {
    match coin {
        Coin::Btc => PAUSED_BY_SIGNER_CHANGE.into(),
        _ => format!(
            "the bridge's signers have changed, so moving coins between {} and {} is paused until \
             the wallet is updated with the new set; sends still work",
            coin.l1().name(),
            coin.vm().name()
        ),
    }
}

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
    if chain.coin() != bridge.coin {
        return invalid(format!(
            "{} isn't one of this bridge's chains",
            chain.name()
        ));
    }
    let to = decode_address(to, &bridge.net)?;
    if bridge.is_peg(&to) {
        let (l1, vm) = (bridge.coin.l1().name(), bridge.coin.vm().name());
        return invalid(if chain.is_vm() {
            format!(
                "that is the bridge's reserve; use Withdraw to {l1}, which tags the payment with your {l1} address"
            )
        } else {
            format!(
                "that is the bridge's own address; use Move to {vm}, which pays your personal deposit address"
            )
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
            fee_rate: bridge.fee_rate,
        },
    )
}

/// A deposit: pays `from`'s personal deposit address on the coin's own
/// chain, and the bridge credits the same address on its VM once the
/// deposit has enough confirmations. The address is derived here from the
/// pinned signers; `told` is the one the bridge gave (`POST
/// /api/deposit-address`, which also registers it), and they must match.
pub fn plan_deposit(
    bridge: &VerifiedBridge,
    from: &Destination,
    coins: Coins,
    amount: u64,
    told: &str,
) -> Result<Plan> {
    let coin = bridge.coin;
    if bridge.signer_change.is_some() {
        return untrusted(paused_by_signer_change(coin));
    }
    let ticker = coin.ticker();
    if amount < bridge.min_deposit {
        return invalid(format!(
            "the smallest deposit is {} {ticker}; a smaller one is not credited",
            coin.format_amount(bridge.min_deposit)
        ));
    }
    if bridge.max_deposit > 0 && amount > bridge.max_deposit {
        return invalid(format!(
            "a deposit can be at most {} {ticker} for now; a larger one is held for a refund",
            coin.format_amount(bridge.max_deposit)
        ));
    }
    let deposit = bridge.signers.deposit_destination_for(coin, from)?;
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
            chain: coin.l1(),
            to: deposit,
            amount,
            data: None,
            fee_rate: bridge.fee_rate,
        },
    )
}

/// A withdrawal: pays the reserve on the VM with a tag (BVMO, DVMO) naming
/// `to` on the coin's own chain. Once the payment is final, the bridge pays
/// the amount there, less that chain's network fee.
pub fn plan_withdrawal(
    bridge: &VerifiedBridge,
    from: &Destination,
    coins: Coins,
    amount: u64,
    to: &str,
) -> Result<Plan> {
    let coin = bridge.coin;
    if bridge.signer_change.is_some() {
        return untrusted(paused_by_signer_change(coin));
    }
    if amount < bridge.min_peg_out {
        return invalid(format!(
            "the smallest withdrawal is {} {}; a smaller one is not paid",
            coin.format_amount(bridge.min_peg_out),
            coin.ticker()
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
            chain: coin.vm(),
            to: bridge.peg.clone(),
            amount,
            data: Some(peg_out_data_for(coin, &dest)),
            fee_rate: bridge.fee_rate,
        },
    )
}
