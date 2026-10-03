//! Checking a change of the peg's signers without trusting the bridge.
//!
//! When BTCVM's operators rotate the signer set, the retired signers move
//! what they hold to the new set (`cmd/btcvm/rotate.go` in btc-vm): the
//! reserve on BTCVM first, then the coins on Bitcoin. Each move spends coins
//! locked by the old set, carries a `BVMM` tag with the SHA-256 of the new
//! set's witness script, and bears the old set's m-of-n signatures with
//! SIGHASH_ALL, which commit to that tag and to every output. A valid move
//! is the old signers themselves saying which set they handed over to: no
//! server can forge it, and the wallet can follow it without being updated.

use crate::bridge::Signers;
use crate::encoding::sha256;
use crate::payment::op_return_data;
use crate::tx::{parse_tx, witness_sighash};
use crate::{Error, Result, untrusted};
use k256::ecdsa::signature::hazmat::PrehashVerifier;
use k256::ecdsa::{Signature, VerifyingKey};
use std::collections::HashMap;

/// Tags a move of the peg's coins to a new signer set; the SHA-256 of the
/// new set's witness script follows.
pub const MIGRATE_TAG: &[u8; 4] = b"BVMM";

/// The hash a move's BVMM tag names, if `tx` carries one: the program of
/// the new set's peg (P2WSH).
pub fn migrate_target(tx: &[u8]) -> Option<[u8; 32]> {
    parse_tx(tx).ok()?.outputs.iter().find_map(|o| {
        op_return_data(&o.script)?
            .strip_prefix(&MIGRATE_TAG[..])?
            .try_into()
            .ok()
    })
}

/// The signer set whose coins `tx` spends from the P2WSH `program`, as the
/// witness script of such an input reveals it. A move from a set the
/// wallet doesn't know yet names its keys this way.
pub fn signers_spent(tx: &[u8], program: &[u8; 32]) -> Option<Signers> {
    parse_tx(tx).ok()?.witnesses.iter().find_map(|w| {
        let script = w.last()?;
        (sha256(script) == *program)
            .then(|| Signers::from_witness_script(script).ok())
            .flatten()
    })
}

/// Checks that `tx` is a move by the signers `from` to the set `to`: it
/// carries the BVMM tag naming `to`, and spends a coin locked by `from` (at
/// its peg or a personal deposit address of it) with `from`'s required
/// signatures, SIGHASH_ALL, in key order as OP_CHECKMULTISIG takes them.
/// `prev_txs` maps the txid of that coin to the transaction that made it,
/// hex, whose bytes must hash to it: its value is part of what was signed.
pub fn verify_move(
    from: &Signers,
    to: &Signers,
    tx: &[u8],
    prev_txs: &HashMap<String, String>,
) -> Result<()> {
    let from_script = from.witness_script()?;
    let target = sha256(&to.witness_script()?);
    if migrate_target(tx) != Some(target) {
        return untrusted(
            "that transaction doesn't move the peg to the new signers (no BVMM tag naming them)",
        );
    }
    let parsed = parse_tx(tx)?;
    let keys = verifying_keys(from)?;
    for (index, witness) in parsed.witnesses.iter().enumerate() {
        let Some((script, items)) = witness.split_last() else {
            continue;
        };
        if !locked_by(script, &from_script) {
            continue;
        }
        let (txid, vout) = &parsed.inputs[index];
        let Some(prev) = prev_txs.get(txid) else {
            continue;
        };
        let prev = hex::decode(prev)
            .map_err(|_| Error::Untrusted(format!("transaction {txid} is not hex")))?;
        let made = parse_tx(&prev)?;
        if made.txid() != *txid {
            return untrusted(format!("the server sent the wrong transaction for {txid}"));
        }
        let Some(coin) = made.outputs.get(*vout as usize) else {
            continue;
        };
        if coin.script != [&[0x00, 0x20][..], &sha256(script)].concat() {
            continue;
        }
        let digest = witness_sighash(tx, index, script, coin.value)?;
        if multisig_signed(usize::from(from.required), &keys, items, &digest) {
            return Ok(());
        }
    }
    untrusted("no input of that transaction carries the old signers' signatures")
}

/// Whether `script` locks coins with `from_script`: it is the peg's own,
/// or a personal deposit address's (`<kind || program> OP_DROP` first).
fn locked_by(script: &[u8], from_script: &[u8]) -> bool {
    if script == from_script {
        return true;
    }
    match script.strip_suffix(from_script) {
        Some([n, data @ .., 0x75]) => usize::from(*n) == data.len() && *n < 0x4c,
        _ => false,
    }
}

fn verifying_keys(signers: &Signers) -> Result<Vec<VerifyingKey>> {
    signers
        .public_keys
        .iter()
        .map(|k| {
            hex::decode(k)
                .ok()
                .and_then(|b| VerifyingKey::from_sec1_bytes(&b).ok())
                .ok_or_else(|| Error::Invalid("signer keys must be compressed public keys".into()))
        })
        .collect()
}

/// OP_CHECKMULTISIG's check of a witness `[dummy, sig…]`: the empty dummy,
/// exactly `required` signatures, each SIGHASH_ALL, each by a later key
/// than the one before.
fn multisig_signed(
    required: usize,
    keys: &[VerifyingKey],
    items: &[Vec<u8>],
    digest: &[u8; 32],
) -> bool {
    let Some((dummy, sigs)) = items.split_first() else {
        return false;
    };
    if !dummy.is_empty() || sigs.len() != required {
        return false;
    }
    let mut next = 0;
    for sig in sigs {
        let Some((&1, der)) = sig.split_last() else {
            return false;
        };
        let Ok(sig) = Signature::from_der(der) else {
            return false;
        };
        let sig = sig.normalize_s().unwrap_or(sig);
        loop {
            let Some(key) = keys.get(next) else {
                return false;
            };
            next += 1;
            if key.verify_prehash(digest, &sig).is_ok() {
                break;
            }
        }
    }
    true
}
