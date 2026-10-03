//! Helpers shared by the integration tests.
#![allow(dead_code)]

use btcvm_wallet_core::{Destination, Utxo, tx};
use std::collections::HashMap;

/// A minimal legacy transaction paying `outputs`, for coins to spend, as the
/// vectors' generator makes them.
pub fn prev_tx(tag: u8, outputs: &[(u64, Vec<u8>)]) -> Vec<u8> {
    let mut raw = vec![1, 0, 0, 0, 1];
    raw.extend([tag; 32]);
    raw.extend([0, 0, 0, 0, 1, 0x51, 0xff, 0xff, 0xff, 0xff]);
    raw.push(outputs.len() as u8);
    for (value, script) in outputs {
        raw.extend(value.to_le_bytes());
        raw.push(script.len() as u8);
        raw.extend(script);
    }
    raw.extend([0, 0, 0, 0]);
    raw
}

/// Coins of `values` paying `to`, from one transaction, as the server lists
/// them, with that transaction's hex by txid.
pub fn coins(to: &Destination, values: &[u64]) -> (Vec<Utxo>, HashMap<String, String>) {
    let outputs: Vec<(u64, Vec<u8>)> = values.iter().map(|&v| (v, to.pk_script())).collect();
    let raw = prev_tx(values.len() as u8, &outputs);
    let txid = tx::txid(&raw).unwrap();
    let utxos = values
        .iter()
        .enumerate()
        .map(|(vout, v)| Utxo {
            txid: txid.clone(),
            vout: vout as u32,
            value: v.to_string(),
            script: hex::encode(to.pk_script()),
            confirmations: 1,
        })
        .collect();
    (utxos, HashMap::from([(txid, hex::encode(&raw))]))
}
