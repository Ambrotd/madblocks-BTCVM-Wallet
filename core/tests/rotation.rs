//! Following a rotation of the peg's signers from the old signers' own
//! signatures, and the BIP 143 digest that checking them needs.

mod common;

use btcvm_wallet_core::bridge::{Pinned, Signers};
use btcvm_wallet_core::encoding::sha256;
use btcvm_wallet_core::rotation::{MIGRATE_TAG, migrate_target, signers_spent, verify_move};
use btcvm_wallet_core::tx::{self, witness_sighash};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{Signature, SigningKey};
use std::collections::HashMap;

/// BIP 143's own examples: a native P2WPKH input (with a lock time), and a
/// 6-of-6 multisig witness script, SIGHASH_ALL.
#[test]
fn bip143_digests() {
    let p2wpkh = hex::decode("0100000002fff7f7881a8099afa6940d42d1e7f6362bec38171ea3edf433541db4e4ad969f0000000000eeffffffef51e1b804cc89d182d279655c3aa89e815b1b309fe287d9b2b55d57b90ec68a0100000000ffffffff02202cb206000000001976a9148280b37df378db99f66f85c95a783a76ac7a6d5988ac9093510d000000001976a9143bde42dbee7e4dbe6a21b2d50ce2f0167faa815988ac11000000").unwrap();
    let code = hex::decode("76a9141d0f172a0ecb48aee1be1f2687d2963ae33f71a188ac").unwrap();
    assert_eq!(
        hex::encode(witness_sighash(&p2wpkh, 1, &code, 600_000_000).unwrap()),
        "c37af31116d1b27caf68aae9e3ac82f1477929014d5b917657d0eb49478cb670"
    );

    let multisig = hex::decode("010000000136641869ca081e70f394c6948e8af409e18b619df2ed74aa106c1ca29787b96e0100000000ffffffff0200e9a435000000001976a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2688acc0832f05000000001976a9147480a33f950689af511e6e84c138dbbd3c3ee41588ac00000000").unwrap();
    let script = hex::decode("56210307b8ae49ac90a048e9b53357a2354b3334e9c8bee813ecb98e99a7e07e8c3ba32103b28f0c28bfab54554ae8c658ac5c3e0ce6e79ad336331f78c428dd43eea8449b21034b8113d703413d57761b8b9781957b8c0ac1dfe69f492580ca4195f50376ba4a21033400f6afecb833092a9a21cfdf1ed1376e58c5d1f47de74683123987e967a8f42103a6d48b1131e94ba04d9737d61acdaa1322008af9602b3b14862c07a1789aac162102d8b661b0b3302ee2f162b09e07a55ad5dfbe673a9f01d9f0c19617681024306b56ae").unwrap();
    assert_eq!(
        hex::encode(witness_sighash(&multisig, 0, &script, 987_654_321).unwrap()),
        "185c0be5263dce5b4bb50a047973c1b6272bfbd0103a89444597dc40b248ee7c"
    );
    // The same script names its set: six keys, all six required.
    let six = Signers::from_witness_script(&script).unwrap();
    assert_eq!((six.required, six.public_keys.len()), (6, 6));
}

#[test]
fn a_signer_set_reads_back_from_its_script() {
    let pinned = Pinned::mainnet().signers;
    let script = pinned.witness_script().unwrap();
    assert_eq!(Signers::from_witness_script(&script).unwrap(), pinned);
    assert!(Signers::from_witness_script(&script[1..]).is_err());
    assert!(Signers::from_witness_script(&[0x51, 0x51, 0xae]).is_err());
}

struct Set {
    keys: Vec<SigningKey>,
    signers: Signers,
}

fn set(required: u8, n: usize) -> Set {
    let keys: Vec<SigningKey> = (0..n)
        .map(|_| SigningKey::random(&mut rand_core::OsRng))
        .collect();
    let signers = Signers {
        required,
        public_keys: keys
            .iter()
            .map(|k| hex::encode(k.verifying_key().to_encoded_point(true).as_bytes()))
            .collect(),
    };
    Set { keys, signers }
}

fn p2wsh(script: &[u8]) -> Vec<u8> {
    [&[0x00, 0x20][..], &sha256(script)].concat()
}

fn tag(to: &Signers) -> Vec<u8> {
    let data = [&MIGRATE_TAG[..], &sha256(&to.witness_script().unwrap())].concat();
    [&[0x6a, data.len() as u8][..], &data].concat()
}

/// A version 2 transaction spending `inputs` (txid, vout) to `outputs`,
/// with `witnesses` if given.
fn transaction(
    inputs: &[(String, u32)],
    outputs: &[(u64, Vec<u8>)],
    witnesses: Option<&[Vec<Vec<u8>>]>,
) -> Vec<u8> {
    let mut raw = 2u32.to_le_bytes().to_vec();
    if witnesses.is_some() {
        raw.extend([0x00, 0x01]);
    }
    raw.push(inputs.len() as u8);
    for (txid, vout) in inputs {
        let mut id = hex::decode(txid).unwrap();
        id.reverse();
        raw.extend(id);
        raw.extend(vout.to_le_bytes());
        raw.push(0);
        raw.extend(0xffff_fffdu32.to_le_bytes());
    }
    raw.push(outputs.len() as u8);
    for (value, script) in outputs {
        raw.extend(value.to_le_bytes());
        raw.push(script.len() as u8);
        raw.extend(script);
    }
    for witness in witnesses.unwrap_or(&[]) {
        raw.push(witness.len() as u8);
        for item in witness {
            raw.push(item.len() as u8);
            raw.extend(item);
        }
    }
    raw.extend(0u32.to_le_bytes());
    raw
}

fn sign(key: &SigningKey, digest: &[u8; 32], hash_type: u8) -> Vec<u8> {
    let sig: Signature = key.sign_prehash(digest).unwrap();
    let sig = sig.normalize_s().unwrap_or(sig);
    [sig.to_der().as_bytes(), &[hash_type]].concat()
}

/// A move of a coin locked by `script` (`old`'s peg or a deposit script of
/// it) to `new`, signed by `old`'s keys `signing` with `hash_type`; the
/// move and the transaction that made the coin, by txid.
fn moved(
    old: &Set,
    script: &[u8],
    new: &Signers,
    signing: &[usize],
    hash_type: u8,
    tagged: &Signers,
) -> (Vec<u8>, HashMap<String, String>) {
    let value = 100_000;
    let made = common::prev_tx(9, &[(value, p2wsh(script))]);
    let txid = tx::txid(&made).unwrap();
    let inputs = [(txid.clone(), 0)];
    let outputs = [
        (value - 1_000, p2wsh(&new.witness_script().unwrap())),
        (0, tag(tagged)),
    ];
    let unsigned = transaction(&inputs, &outputs, None);
    let digest = witness_sighash(&unsigned, 0, script, value).unwrap();
    let mut witness = vec![Vec::new()];
    witness.extend(
        signing
            .iter()
            .map(|&i| sign(&old.keys[i], &digest, hash_type)),
    );
    witness.push(script.to_vec());
    let signed = transaction(&inputs, &outputs, Some(&[witness]));
    (signed, HashMap::from([(txid, hex::encode(made))]))
}

#[test]
fn a_move_signed_by_the_old_set_proves_the_new_one() {
    let old = set(2, 3);
    let new = set(2, 3).signers;
    let script = old.signers.witness_script().unwrap();
    let (tx, prev) = moved(&old, &script, &new, &[0, 2], 1, &new);
    verify_move(&old.signers, &new, &tx, &prev).unwrap();
    assert_eq!(
        migrate_target(&tx),
        Some(sha256(&new.witness_script().unwrap()))
    );
    // Its witness names the old set, for a wallet that didn't know it.
    assert_eq!(
        signers_spent(&tx, &sha256(&script)),
        Some(old.signers.clone())
    );
    // Any two of the three, in key order.
    for pair in [[0, 1], [1, 2]] {
        let (tx, prev) = moved(&old, &script, &new, &pair, 1, &new);
        verify_move(&old.signers, &new, &tx, &prev).unwrap();
    }
    // A coin at a personal deposit address of the old set counts too.
    let deposit = [&[21u8][..], &[2; 21], &[0x75], &script].concat();
    let (tx, prev) = moved(&old, &deposit, &new, &[0, 1], 1, &new);
    verify_move(&old.signers, &new, &tx, &prev).unwrap();
}

#[test]
fn nothing_else_passes_for_a_move() {
    let old = set(2, 3);
    let new = set(2, 3).signers;
    let other = set(2, 3).signers;
    let script = old.signers.witness_script().unwrap();
    let refused = |tx: &[u8], prev: &HashMap<String, String>| {
        verify_move(&old.signers, &new, tx, prev).is_err()
    };

    // Signatures out of key order, or too few.
    let (tx, prev) = moved(&old, &script, &new, &[2, 0], 1, &new);
    assert!(refused(&tx, &prev));
    let (tx, prev) = moved(&old, &script, &new, &[1], 1, &new);
    assert!(refused(&tx, &prev));
    // Signatures that don't commit to the outputs (SIGHASH_NONE).
    let (tx, prev) = moved(&old, &script, &new, &[0, 1], 2, &new);
    assert!(refused(&tx, &prev));
    // A tag naming another set than the one the bridge reports.
    let (tx, prev) = moved(&old, &script, &new, &[0, 1], 1, &other);
    assert!(refused(&tx, &prev));
    // Signed by a set that isn't the trusted one.
    let stranger = set(2, 3);
    let theirs = stranger.signers.witness_script().unwrap();
    let (tx, prev) = moved(&stranger, &theirs, &new, &[0, 1], 1, &new);
    assert!(refused(&tx, &prev));
    // The coin's transaction missing, or not the one its txid names.
    let (tx, prev) = moved(&old, &script, &new, &[0, 1], 1, &new);
    assert!(refused(&tx, &HashMap::new()));
    let swapped: HashMap<String, String> = prev
        .keys()
        .map(|k| {
            (
                k.clone(),
                hex::encode(common::prev_tx(8, &[(100_000, p2wsh(&script))])),
            )
        })
        .collect();
    assert!(refused(&tx, &swapped));
    // A transaction with no tag at all.
    assert!(
        migrate_target(
            &prev
                .values()
                .map(|h| hex::decode(h).unwrap())
                .next()
                .unwrap()
        )
        .is_none()
    );
}
