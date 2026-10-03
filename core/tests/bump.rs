//! Replacing a stalled Bitcoin payment with a higher fee (BIP 125).

mod common;

use btcvm_wallet_core::payment::DUST;
use btcvm_wallet_core::tx::parse_tx;
use btcvm_wallet_core::{
    Chain, Key, MAINNET, Request, decode_address, plan_bump, plan_payment, sign_plan,
};
use std::collections::HashMap;

const TO: &str = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";

/// A signed payment of `amount` at `rate` from coins of `values`, with the
/// transactions that made them.
fn paid(
    key: &Key,
    values: &[u64],
    amount: u64,
    rate: u64,
) -> (Vec<u8>, HashMap<String, String>, u64) {
    let from = key.destination();
    let (utxos, raws) = common::coins(&from, values);
    let plan = plan_payment(
        &from,
        &utxos,
        &raws,
        &Request {
            chain: Chain::Bitcoin,
            to: decode_address(TO, &MAINNET).unwrap(),
            amount,
            data: None,
            fee_rate: rate,
        },
    )
    .unwrap();
    let signed = sign_plan(key, &plan).unwrap();
    (hex::decode(&signed.hex).unwrap(), raws, plan.fee)
}

#[test]
fn a_higher_fee_comes_out_of_the_change() {
    let key = Key::generate();
    let (original, raws, old_fee) = paid(&key, &[100_000], 30_000, 2);
    let before = parse_tx(&original).unwrap();
    let plan = plan_bump(&key.destination(), &original, &raws, 10).unwrap();
    assert!(plan.fee > old_fee);
    // The same coins and the same payment; only the change is smaller.
    assert_eq!(
        plan.spends(),
        before
            .inputs
            .iter()
            .map(|(t, v)| format!("{t}:{v}"))
            .collect::<Vec<_>>()
    );
    let after = plan.outputs();
    assert_eq!(after.len(), before.outputs.len());
    assert_eq!(after[0], before.outputs[0]);
    assert_eq!(after[1].script, before.outputs[1].script);
    assert_eq!(before.outputs[1].value - after[1].value, plan.fee - old_fee);
    // It signs, to a new transaction that still signals BIP 125.
    let signed = sign_plan(&key, &plan).unwrap();
    let replaced = parse_tx(&hex::decode(&signed.hex).unwrap()).unwrap();
    assert_ne!(replaced.txid(), before.txid());
    assert!(replaced.sequences.iter().all(|&s| s < 0xffff_fffe));
    assert_eq!(replaced.outputs, plan.outputs());
}

#[test]
fn a_replacement_always_pays_more_than_its_size() {
    let key = Key::generate();
    let (original, raws, old_fee) = paid(&key, &[100_000], 30_000, 5);
    // The same rate, or a lower one: still more than before, by 1 sat/vB of
    // its size, as nodes require.
    for rate in [1, 5] {
        let plan = plan_bump(&key.destination(), &original, &raws, rate).unwrap();
        let size = old_fee / 5;
        assert!(
            plan.fee >= old_fee + size,
            "{rate}: {} vs {old_fee}",
            plan.fee
        );
    }
}

#[test]
fn what_cant_be_replaced_is_refused() {
    let key = Key::generate();
    let from = key.destination();

    // Everything sent: no change to take a higher fee from.
    let (utxos, raws) = common::coins(&from, &[50_000]);
    let all = btcvm_wallet_core::max_payment(
        &from,
        &utxos,
        &raws,
        &Request {
            chain: Chain::Bitcoin,
            to: decode_address(TO, &MAINNET).unwrap(),
            amount: 0,
            data: None,
            fee_rate: 2,
        },
    )
    .unwrap();
    let (no_change, raws_nc, _) = paid(&key, &[50_000], all, 2);
    assert!(plan_bump(&from, &no_change, &raws_nc, 10).is_err());

    // Change too small to pay the extra.
    let (small, raws_small, _) = paid(&key, &[31_000], 30_000, 1);
    assert!(plan_bump(&from, &small, &raws_small, 100).is_err());

    // Coins that aren't the wallet's.
    let (original, raws, _) = paid(&key, &[100_000], 30_000, 2);
    let stranger = Key::generate().destination();
    assert!(plan_bump(&stranger, &original, &raws, 10).is_err());

    // A wrong transaction for a coin.
    let (_, other_raws) = common::coins(&from, &[100_001]);
    let mut lying: HashMap<String, String> = raws
        .keys()
        .map(|k| (k.clone(), other_raws.values().next().unwrap().clone()))
        .collect();
    assert!(
        plan_bump(&from, &original, &lying, 10)
            .unwrap_err()
            .is_untrusted()
    );
    lying.clear();
    assert!(plan_bump(&from, &original, &lying, 10).is_err());

    // A fee rate out of bounds.
    assert!(plan_bump(&from, &original, &raws, 0).is_err());
    assert!(plan_bump(&from, &original, &raws, 1_001).is_err());

    // A payment that doesn't signal BIP 125 (its sequences set to final).
    let parsed = parse_tx(&original).unwrap();
    let mut final_seq = original.clone();
    // After version (4), marker and flag (2), the input count (1), each
    // input's outpoint (36) and empty script (1) come its sequence.
    let mut at = 4 + 2 + 1;
    for _ in &parsed.inputs {
        at += 36 + 1;
        final_seq[at..at + 4].copy_from_slice(&[0xff; 4]);
        at += 4;
    }
    assert!(plan_bump(&from, &final_seq, &raws, 10).is_err());
}

#[test]
fn change_never_drops_below_dust() {
    let key = Key::generate();
    let (original, raws, old_fee) = paid(&key, &[40_000], 30_000, 2);
    let change = parse_tx(&original).unwrap().outputs[1].value;
    // The highest rate the change can pay while keeping dust.
    let size = old_fee / 2;
    let most = (change + old_fee - DUST) / size;
    let plan = plan_bump(&key.destination(), &original, &raws, most).unwrap();
    assert!(plan.outputs()[1].value >= DUST);
    assert!(plan_bump(&key.destination(), &original, &raws, most + 1).is_err());
}
