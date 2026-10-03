//! The wallets end to end, with stand-ins for the bridge's server and for
//! Windows Hello: making and backing up wallets, paying, tracking payments,
//! speeding one up, and following a rotation of the bridge's signers.

use crate::api::{
    AddressView, Api, ApiError, BridgeStatus, DepositEntry, FeeEstimates, HistoryEntry, ListedTx,
    PegOutStatus, Prices,
};
use crate::wallet::{BackupText, Wallet};
use btcvm_wallet_core::bridge::{Pinned, Signers};
use btcvm_wallet_core::encoding::sha256;
use btcvm_wallet_core::rotation::MIGRATE_TAG;
use btcvm_wallet_core::tx::{self, witness_sighash};
use btcvm_wallet_core::{BridgeInfo, Chain, Key, MAINNET, Utxo, decode_address, format_btc};
use btcvm_wallet_vault::{Gate, VaultError};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{Signature, SigningKey};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

const TO: &str = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";

// --- Windows Hello's stand-in ------------------------------------------------------

/// Each key a secret, its "public key" the secret's hash, its "signature" a
/// hash of the secret and the message; it counts the times it would ask.
#[derive(Default)]
struct FakeGate {
    keys: Mutex<HashMap<String, [u8; 32]>>,
    prompts: Mutex<u32>,
}

impl FakeGate {
    fn prompts(&self) -> u32 {
        *self.prompts.lock().unwrap()
    }
}

impl Gate for FakeGate {
    fn kind(&self) -> &'static str {
        "windows-hello"
    }

    fn create(&self, name: &str) -> Result<Vec<u8>, VaultError> {
        *self.prompts.lock().unwrap() += 1;
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        self.keys.lock().unwrap().insert(name.into(), secret);
        Ok(Sha256::digest(secret).to_vec())
    }

    fn sign(
        &self,
        name: &str,
        challenge: &[u8],
        public_key: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let secret = *self
            .keys
            .lock()
            .unwrap()
            .get(name)
            .ok_or(VaultError::GateMissing)?;
        if Sha256::digest(secret)[..] != *public_key {
            return Err(VaultError::GateMissing);
        }
        *self.prompts.lock().unwrap() += 1;
        Ok(Zeroizing::new(
            Sha256::new()
                .chain_update(secret)
                .chain_update(challenge)
                .finalize()
                .to_vec(),
        ))
    }

    fn delete(&self, name: &str) -> Result<(), VaultError> {
        self.keys.lock().unwrap().remove(name);
        Ok(())
    }
}

// --- the bridge's stand-in ---------------------------------------------------------

#[derive(Default)]
struct State {
    info: serde_json::Value,
    utxos: HashMap<(Chain, String), Vec<Utxo>>,
    history: HashMap<(Chain, String), Vec<HistoryEntry>>,
    raw: HashMap<String, String>,
    sent: Vec<(Chain, String)>,
    /// The status a broadcast fails with, if set.
    refuse: Option<u16>,
    /// A deposit address to answer instead of the right one.
    wrong_deposit_address: Option<String>,
    /// What mempool.space says an address holds, when it isn't the coins.
    second_opinion: Option<u64>,
}

#[derive(Default)]
struct FakeApi {
    state: Mutex<State>,
}

fn missing(what: &str) -> ApiError {
    ApiError {
        status: 404,
        message: format!("no {what}"),
    }
}

impl Api for FakeApi {
    fn base(&self) -> &str {
        "https://bridge.test"
    }

    fn info(&self) -> Result<BridgeInfo, ApiError> {
        Ok(serde_json::from_value(self.state.lock().unwrap().info.clone()).unwrap())
    }

    fn status(&self) -> Result<BridgeStatus, ApiError> {
        Ok(BridgeStatus::default())
    }

    fn address(&self, chain: Chain, address: &str) -> Result<AddressView, ApiError> {
        let s = self.state.lock().unwrap();
        let key = (chain, address.to_string());
        let utxos = s.utxos.get(&key).cloned().unwrap_or_default();
        let confirmed: u64 = utxos.iter().map(|u| u.value.parse::<u64>().unwrap()).sum();
        Ok(AddressView {
            confirmed: format_btc(confirmed),
            pending: "0".into(),
            utxos,
            history: s.history.get(&key).cloned().unwrap_or_default(),
        })
    }

    fn watch_bitcoin(&self, _: &str) -> Result<(), ApiError> {
        Ok(())
    }

    fn raw_tx(&self, _: Chain, txid: &str) -> Result<String, ApiError> {
        self.state
            .lock()
            .unwrap()
            .raw
            .get(txid)
            .cloned()
            .ok_or_else(|| missing("transaction"))
    }

    fn broadcast(&self, chain: Chain, hex: &str) -> Result<String, ApiError> {
        let mut s = self.state.lock().unwrap();
        if let Some(status) = s.refuse {
            return Err(ApiError {
                status,
                message: format!("refused with {status}"),
            });
        }
        let txid = tx::txid(&hex::decode(hex).unwrap()).unwrap();
        s.raw.insert(txid.clone(), hex.to_string());
        s.sent.push((chain, txid.clone()));
        Ok(txid)
    }

    fn deposit_address(&self, address: &str) -> Result<String, ApiError> {
        let s = self.state.lock().unwrap();
        if let Some(wrong) = &s.wrong_deposit_address {
            return Ok(wrong.clone());
        }
        let signers: Signers = serde_json::from_value(s.info["signers"].clone()).unwrap();
        let dest = decode_address(address, &MAINNET).unwrap();
        Ok(signers
            .deposit_destination(&dest)
            .unwrap()
            .address(&MAINNET))
    }

    fn deposits(&self, _: &str) -> Result<Vec<DepositEntry>, ApiError> {
        Ok(Vec::new())
    }

    fn peg_out(&self, _: &str) -> Result<PegOutStatus, ApiError> {
        Ok(PegOutStatus {
            status: "pending".into(),
            pays: None,
            payment_txid: None,
            payment_confirmations: None,
        })
    }

    fn events(&self) -> Result<Box<dyn BufRead + Send>, ApiError> {
        Err(missing("event stream"))
    }

    fn recommended_fees(&self) -> Result<FeeEstimates, ApiError> {
        Err(missing("fee estimates"))
    }

    fn prices(&self) -> Result<Prices, ApiError> {
        Ok(Prices {
            usd: 60_000.0,
            eur: 55_000.0,
        })
    }

    fn bitcoin_address_txs(&self, _: &str) -> Result<Vec<ListedTx>, ApiError> {
        Ok(Vec::new())
    }

    fn bitcoin_balance(&self, address: &str) -> Result<u64, ApiError> {
        let s = self.state.lock().unwrap();
        Ok(s.second_opinion.unwrap_or_else(|| {
            s.utxos
                .get(&(Chain::Bitcoin, address.to_string()))
                .map(|u| u.iter().map(|c| c.value.parse::<u64>().unwrap()).sum())
                .unwrap_or(0)
        }))
    }
}

impl FakeApi {
    fn with(signers: &Signers) -> Arc<FakeApi> {
        let api = Arc::new(FakeApi::default());
        api.report(signers);
        api
    }

    /// The bridge reports `signers`, its peg and reserve following from them.
    fn report(&self, signers: &Signers) {
        let mut info: serde_json::Value = serde_json::from_str(include_str!(
            "../../core/tests/fixtures/metalbtc-info-2026-10-03.json"
        ))
        .unwrap();
        let peg = signers.peg().unwrap().address(&MAINNET);
        info["signers"] = serde_json::to_value(signers).unwrap();
        info["pegAddress"] = peg.clone().into();
        info["reserveAddress"] = peg.into();
        self.state.lock().unwrap().info = info;
    }

    /// Confirmed coins of `values` paying `address` on `chain`.
    fn fund(&self, chain: Chain, address: &str, values: &[u64]) {
        let script = decode_address(address, &MAINNET).unwrap().pk_script();
        let mut tag = [0u8; 1];
        OsRng.fill_bytes(&mut tag);
        let raw = legacy_tx(
            tag[0],
            &values
                .iter()
                .map(|&v| (v, script.clone()))
                .collect::<Vec<_>>(),
        );
        let txid = tx::txid(&raw).unwrap();
        let mut s = self.state.lock().unwrap();
        s.raw.insert(txid.clone(), hex::encode(&raw));
        let coins = s.utxos.entry((chain, address.to_string())).or_default();
        for (vout, v) in values.iter().enumerate() {
            coins.push(Utxo {
                txid: txid.clone(),
                vout: vout as u32,
                value: v.to_string(),
                script: hex::encode(&script),
                confirmations: 1,
            });
        }
    }

    /// `txid` in `address`'s history on `chain`.
    fn seen(&self, chain: Chain, address: &str, txid: &str, net: &str, confirmations: i64) {
        let mut s = self.state.lock().unwrap();
        let history = s.history.entry((chain, address.to_string())).or_default();
        history.retain(|h| h.txid != txid);
        history.insert(
            0,
            HistoryEntry {
                txid: txid.into(),
                net: net.into(),
                confirmations,
                time: Some(1_790_000_000),
            },
        );
    }

    fn sent(&self) -> Vec<(Chain, String)> {
        self.state.lock().unwrap().sent.clone()
    }
}

/// A minimal legacy transaction paying `outputs`, to make coins.
fn legacy_tx(tag: u8, outputs: &[(u64, Vec<u8>)]) -> Vec<u8> {
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

// --- signer sets with known keys -----------------------------------------------------

struct Set {
    keys: Vec<SigningKey>,
    signers: Signers,
}

fn set() -> Set {
    let keys: Vec<SigningKey> = (0..3).map(|_| SigningKey::random(&mut OsRng)).collect();
    let signers = Signers {
        required: 2,
        public_keys: keys
            .iter()
            .map(|k| hex::encode(k.verifying_key().to_encoded_point(true).as_bytes()))
            .collect(),
    };
    Set { keys, signers }
}

fn pinned(signers: &Signers) -> Pinned {
    Pinned {
        signers: signers.clone(),
        ..Pinned::mainnet()
    }
}

/// The old set's move of its reserve on BTCVM to the new set, as btc-vm
/// makes one: the move and the transaction that made the coin it spends.
fn handover(old: &Set, new: &Signers) -> ((String, String), (String, String)) {
    let script = old.signers.witness_script().unwrap();
    let p2wsh = |s: &[u8]| [&[0x00, 0x20][..], &sha256(s)].concat();
    let made = legacy_tx(7, &[(100_000, p2wsh(&script))]);
    let made_id = tx::txid(&made).unwrap();
    let tag_data = [&MIGRATE_TAG[..], &sha256(&new.witness_script().unwrap())].concat();
    let outputs = vec![
        (99_000u64, p2wsh(&new.witness_script().unwrap())),
        (0, [&[0x6a, tag_data.len() as u8][..], &tag_data].concat()),
    ];
    let unsigned = segwit_tx(&made_id, &outputs, None);
    let digest = witness_sighash(&unsigned, 0, &script, 100_000).unwrap();
    let sign = |k: &SigningKey| {
        let sig: Signature = k.sign_prehash(&digest).unwrap();
        let sig = sig.normalize_s().unwrap_or(sig);
        [sig.to_der().as_bytes(), &[1]].concat()
    };
    let witness = vec![Vec::new(), sign(&old.keys[0]), sign(&old.keys[1]), script];
    let signed = segwit_tx(&made_id, &outputs, Some(&witness));
    let id = tx::txid(&signed).unwrap();
    ((id, hex::encode(signed)), (made_id, hex::encode(made)))
}

fn segwit_tx(spends: &str, outputs: &[(u64, Vec<u8>)], witness: Option<&[Vec<u8>]>) -> Vec<u8> {
    let mut raw = 2u32.to_le_bytes().to_vec();
    if witness.is_some() {
        raw.extend([0x00, 0x01]);
    }
    raw.push(1);
    let mut id = hex::decode(spends).unwrap();
    id.reverse();
    raw.extend(id);
    raw.extend(0u32.to_le_bytes());
    raw.push(0);
    raw.extend(0xffff_fffdu32.to_le_bytes());
    raw.push(outputs.len() as u8);
    for (value, script) in outputs {
        raw.extend(value.to_le_bytes());
        raw.push(script.len() as u8);
        raw.extend(script);
    }
    if let Some(items) = witness {
        raw.push(items.len() as u8);
        for item in items {
            raw.push(item.len() as u8);
            raw.extend(item);
        }
    }
    raw.extend(0u32.to_le_bytes());
    raw
}

// --- the wallets ---------------------------------------------------------------------

fn temp_dir() -> PathBuf {
    let mut id = [0u8; 8];
    OsRng.fill_bytes(&mut id);
    let dir = std::env::temp_dir().join(format!("btcvm-app-test-{}", hex::encode(id)));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open(
    dir: &std::path::Path,
    api: &Arc<FakeApi>,
    gate: &Arc<FakeGate>,
    trusted: &Signers,
) -> Wallet {
    let api = api.clone();
    Wallet::open_with(
        dir.to_path_buf(),
        "en",
        Box::new(move |_| api.clone() as Arc<dyn Api>),
        gate.clone(),
        pinned(trusted),
    )
}

/// A wallet holding `key`, imported (so already backed up).
fn with_key(key: &Key) -> (Wallet, Arc<FakeApi>, Arc<FakeGate>, Set) {
    let trusted = set();
    let api = FakeApi::with(&trusted.signers);
    let gate = Arc::new(FakeGate::default());
    let w = open(&temp_dir(), &api, &gate, &trusted.signers);
    w.import("", Zeroizing::new(key.wif(&MAINNET).to_string()))
        .unwrap();
    (w, api, gate, trusted)
}

fn address(key: &Key) -> String {
    key.destination().address(&MAINNET)
}

#[test]
fn a_new_wallet_has_twelve_words_and_receives_once_they_are_checked() {
    let trusted = set();
    let api = FakeApi::with(&trusted.signers);
    let gate = Arc::new(FakeGate::default());
    let w = open(&temp_dir(), &api, &gate, &trusted.signers);
    let shown: RefCell<Option<String>> = RefCell::new(None);
    let view = w
        .create("Savings", |b: &BackupText| {
            *shown.borrow_mut() = b.words.as_ref().map(|w| w.to_string());
            assert!(b.wif.starts_with('K') || b.wif.starts_with('L'));
            true
        })
        .unwrap();
    assert_eq!(
        gate.prompts(),
        2,
        "making the Windows Hello key, then sealing"
    );
    let words: Vec<String> = shown
        .borrow()
        .clone()
        .unwrap()
        .split(' ')
        .map(String::from)
        .collect();
    assert_eq!(words.len(), 12);
    // Nothing can be received until the words are typed back.
    assert!(view.receive_blocked && view.address.is_none());
    let check = view.backup_check.clone().unwrap();
    assert_eq!((check.kind, check.items.len()), ("words", 3));
    let answers: Vec<String> = check
        .items
        .iter()
        .map(|[n, _]| words[n - 1].clone())
        .collect();
    let mut wrong = answers.clone();
    wrong[1] = "zoo".into();
    assert!(w.verify_backup(&wrong).is_err());
    // Any case, or the first four letters of each.
    let typed: Vec<String> = answers
        .iter()
        .map(|a| a[..a.len().min(4)].to_uppercase())
        .collect();
    let view = w.verify_backup(&typed).unwrap();
    assert!(!view.receive_blocked && view.address.is_some() && view.backup_check.is_none());
    assert_eq!(view.wallets[0].name, "Savings");
}

#[test]
fn more_wallets_share_the_windows_hello_key_and_a_key_is_kept_once() {
    let trusted = set();
    let api = FakeApi::with(&trusted.signers);
    let gate = Arc::new(FakeGate::default());
    let w = open(&temp_dir(), &api, &gate, &trusted.signers);
    w.create("", |_| false).unwrap();
    assert_eq!(gate.prompts(), 2);
    let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
    w.import("Phrase", Zeroizing::new(phrase.into())).unwrap();
    assert_eq!(gate.prompts(), 3, "a second wallet asks once");
    let key = Key::generate();
    let view = w
        .import("Key", Zeroizing::new(key.wif(&MAINNET).to_string()))
        .unwrap();
    assert_eq!(gate.prompts(), 4);
    assert_eq!(view.wallets.len(), 3);
    assert_eq!(view.address.as_deref(), Some(address(&key).as_str()));
    // The same key again is refused, as WIF or as its phrase.
    assert!(
        w.import("", Zeroizing::new(key.wif(&MAINNET).to_string()))
            .is_err()
    );
    assert!(w.import("", Zeroizing::new(phrase.to_uppercase())).is_err());
    // Removing one keeps the shared Windows Hello key for the others.
    w.remove(|| true).unwrap();
    assert_eq!(gate.keys.lock().unwrap().len(), 1);
    let first = w.view().wallets[0].id.clone();
    w.select(&first).unwrap();
    assert!(w.backup(|_| false).is_ok(), "the first wallet still opens");
}

#[test]
fn a_key_wallet_is_checked_by_characters() {
    let key = Key::generate();
    let (w, _, _, _) = with_key(&key);
    let view = w.backup(|b| b.words.is_none()).unwrap();
    let check = view.backup_check.unwrap();
    assert_eq!((check.kind, check.items.len()), ("chars", 2));
    let wif = key.wif(&MAINNET);
    let answers: Vec<String> = check
        .items
        .iter()
        .map(|[a, b]| wif[a - 1..*b].to_string())
        .collect();
    let mut wrong = answers.clone();
    wrong[0] = if wrong[0] == "1111" {
        "2222".into()
    } else {
        "1111".into()
    };
    assert!(w.verify_backup(&wrong).is_err());
    let view = w.verify_backup(&answers).unwrap();
    assert!(view.backup_check.is_none());
}

#[test]
fn payments_are_reviewed_signed_tracked_and_settled() {
    let key = Key::generate();
    let (w, api, gate, _) = with_key(&key);
    let me = address(&key);
    api.fund(Chain::Bitcoin, &me, &[100_000]);
    w.refresh();
    let review = w.prepare_send("bitcoin", TO, "0.0003", Some(3)).unwrap();
    assert_eq!(
        (review.kind, review.chain, review.fee_rate),
        ("send", "bitcoin", Some(3))
    );
    assert_eq!(review.to_known, "new");
    assert_eq!(
        review.outputs.iter().map(|o| o.role).collect::<Vec<_>>(),
        ["pay", "change"]
    );
    let before = gate.prompts();
    let sent = w.confirm(review.id).unwrap();
    assert_eq!(
        gate.prompts(),
        before + 1,
        "one Windows Hello prompt to sign"
    );
    assert_eq!(api.sent(), [(Chain::Bitcoin, sent.txid.clone())]);
    // The payment is tracked, and its coin isn't offered again.
    assert_eq!(w.view().in_flight.len(), 1);
    assert!(w.prepare_send("bitcoin", TO, "0.0001", Some(3)).is_err());
    // Confirmed on the chain, it settles.
    api.seen(Chain::Bitcoin, &me, &sent.txid, "-0.00030423", 1);
    w.refresh();
    assert!(w.view().in_flight.is_empty());
}

#[test]
fn a_refused_payment_frees_its_coins_and_an_unanswered_one_stays_tracked() {
    let key = Key::generate();
    let (w, api, _, _) = with_key(&key);
    api.fund(Chain::Btcvm, &address(&key), &[50_000]);
    w.refresh();
    api.state.lock().unwrap().refuse = Some(400);
    let review = w.prepare_send("btcvm", TO, "0.0002", None).unwrap();
    assert!(w.confirm(review.id).is_err());
    assert!(w.view().in_flight.is_empty(), "refused: nothing was sent");
    api.state.lock().unwrap().refuse = Some(502);
    let review = w.prepare_send("btcvm", TO, "0.0002", None).unwrap();
    let e = w.confirm(review.id).unwrap_err();
    assert!(e.message.contains("may or may not have been sent"));
    assert_eq!(
        w.view().in_flight.len(),
        1,
        "no answer: kept, its coins held"
    );
}

#[test]
fn the_address_book_and_own_wallets_are_recognized() {
    let key = Key::generate();
    let (w, api, _, _) = with_key(&key);
    api.fund(Chain::Bitcoin, &address(&key), &[100_000]);
    w.refresh();
    w.add_contact("Exchange", TO, "bitcoin").unwrap();
    let r = w.prepare_send("bitcoin", TO, "0.0001", Some(2)).unwrap();
    assert_eq!(
        (r.to_known, r.to_name.as_deref()),
        ("book", Some("Exchange"))
    );
    api.fund(Chain::Btcvm, &address(&key), &[100_000]);
    w.refresh();
    let r = w.prepare_send("btcvm", TO, "0.0001", None).unwrap();
    assert_eq!(
        r.to_known, "bookOtherChain",
        "saved for Bitcoin, paid on BTCVM"
    );
    assert!(w.add_contact("Again", TO, "bitcoin").is_err());
}

#[test]
fn deposits_go_only_to_the_address_the_trusted_signers_make() {
    let key = Key::generate();
    let (w, api, _, _) = with_key(&key);
    api.fund(Chain::Bitcoin, &address(&key), &[100_000]);
    w.refresh();
    let r = w.prepare_deposit("0.0005", Some(2)).unwrap();
    assert_eq!(r.kind, "deposit");
    assert!(r.outputs.iter().any(|o| o.role == "deposit"));
    api.state.lock().unwrap().wrong_deposit_address = Some(TO.into());
    let e = w.prepare_deposit("0.0005", Some(2)).unwrap_err();
    assert!(e.untrusted, "{}", e.message);
}

#[test]
fn a_stalled_payment_is_sped_up_and_replaces_the_original() {
    let key = Key::generate();
    let (w, api, _, _) = with_key(&key);
    api.fund(Chain::Bitcoin, &address(&key), &[100_000]);
    w.refresh();
    let review = w.prepare_send("bitcoin", TO, "0.0003", Some(2)).unwrap();
    let first = w.confirm(review.id).unwrap().txid;
    let bump = w.prepare_bump(&first, 12).unwrap();
    assert_eq!((bump.kind, bump.fee_rate), ("bump", Some(12)));
    assert!(bump.previous_fee.is_some());
    let second = w.confirm(bump.id).unwrap().txid;
    assert_ne!(first, second);
    let in_flight: Vec<String> = w.view().in_flight.into_iter().map(|o| o.txid).collect();
    assert_eq!(in_flight, [second], "only the replacement is tracked");
}

#[test]
fn max_is_what_a_payment_can_move() {
    let key = Key::generate();
    let (w, api, _, _) = with_key(&key);
    api.fund(Chain::Bitcoin, &address(&key), &[60_000, 40_000]);
    w.refresh();
    let max = w.max_amount("send", "bitcoin", TO, Some(2)).unwrap();
    let review = w.prepare_send("bitcoin", TO, &max, Some(2)).unwrap();
    assert_eq!(review.outputs.len(), 1, "everything sent: no change");
}

#[test]
fn payments_received_are_announced_once() {
    let key = Key::generate();
    let (w, api, _, _) = with_key(&key);
    let me = address(&key);
    api.seen(Chain::Btcvm, &me, &"11".repeat(32), "0.001", 1);
    w.refresh();
    assert!(
        w.take_notices().is_empty(),
        "what was there at first isn't news"
    );
    api.seen(Chain::Btcvm, &me, &"22".repeat(32), "0.0005", 0);
    api.seen(Chain::Btcvm, &me, &"33".repeat(32), "-0.0001", 1);
    w.refresh();
    let notices = w.take_notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(
        (
            notices[0].chain,
            notices[0].amount.as_str(),
            notices[0].confirmed
        ),
        ("btcvm", "0.0005", false)
    );
    w.refresh();
    assert!(w.take_notices().is_empty());
}

#[test]
fn a_signer_change_pauses_moves_until_the_old_signers_hand_over() {
    let old = set();
    let new = set().signers;
    let api = FakeApi::with(&old.signers);
    let gate = Arc::new(FakeGate::default());
    let dir = temp_dir();
    let w = open(&dir, &api, &gate, &old.signers);
    let key = Key::generate();
    w.import("", Zeroizing::new(key.wif(&MAINNET).to_string()))
        .unwrap();
    api.fund(Chain::Bitcoin, &address(&key), &[100_000]);

    // The bridge reports another set, with no move to show for it yet.
    api.report(&new);
    w.refresh();
    assert!(w.view().bridge.signer_change.is_some());
    assert!(
        w.prepare_deposit("0.0005", Some(2)).is_err(),
        "deposits pause"
    );
    assert!(
        w.prepare_send("bitcoin", TO, "0.0001", Some(2)).is_ok(),
        "sends don't"
    );

    // The old signers' move appears among the old reserve's spends on BTCVM.
    let ((move_id, move_hex), (made_id, made_hex)) = handover(&old, &new);
    {
        let mut s = api.state.lock().unwrap();
        s.raw.insert(move_id.clone(), move_hex);
        s.raw.insert(made_id.clone(), made_hex);
    }
    let old_peg = old.signers.peg().unwrap().address(&MAINNET);
    api.seen(Chain::Btcvm, &old_peg, &move_id, "-0.001", 1);
    let view = w.check_rotation_now();
    assert!(view.bridge.signer_change.is_none(), "followed the rotation");
    let rotation = view.bridge.rotation.clone().unwrap();
    assert_eq!(
        (rotation.txid.as_str(), rotation.chain.as_str()),
        (move_id.as_str(), "btcvm")
    );
    assert!(
        w.prepare_deposit("0.0005", Some(2)).is_ok(),
        "deposits resume"
    );
    w.rotation_seen();
    assert!(w.view().bridge.rotation.is_none());
    drop(w);

    // After a restart the stored move is checked again, offline.
    {
        let mut s = api.state.lock().unwrap();
        s.raw.remove(&move_id);
        s.raw.remove(&made_id);
        s.history.clear();
    }
    let w = open(&dir, &api, &gate, &old.signers);
    w.refresh();
    assert!(w.view().bridge.signer_change.is_none());

    // A forged proof in the settings file doesn't survive a restart.
    drop(w);
    let file = dir.join("settings.json");
    let text = std::fs::read_to_string(&file).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
    json["rotations"][0]["tx"] = "00".into();
    std::fs::write(&file, json.to_string()).unwrap();
    let w = open(&dir, &api, &gate, &old.signers);
    w.refresh();
    assert!(w.view().bridge.signer_change.is_some(), "paused again");
}

#[test]
fn a_move_by_anyone_else_is_not_followed() {
    let old = set();
    let impostor = set();
    let new = set().signers;
    let api = FakeApi::with(&old.signers);
    let gate = Arc::new(FakeGate::default());
    let w = open(&temp_dir(), &api, &gate, &old.signers);
    api.report(&new);
    // A "move" from another set's coins, tagged for the new set, shown as
    // spent from the old reserve.
    let ((move_id, move_hex), (made_id, made_hex)) = handover(&impostor, &new);
    {
        let mut s = api.state.lock().unwrap();
        s.raw.insert(move_id.clone(), move_hex);
        s.raw.insert(made_id, made_hex);
    }
    api.seen(
        Chain::Btcvm,
        &old.signers.peg().unwrap().address(&MAINNET),
        &move_id,
        "-0.001",
        1,
    );
    let view = w.check_rotation_now();
    assert!(view.bridge.signer_change.is_some(), "still paused");
    assert!(view.bridge.rotation.is_none());
}

#[test]
fn a_second_opinion_on_the_bitcoin_balance_is_asked_only_when_wanted() {
    let key = Key::generate();
    let (w, api, _, _) = with_key(&key);
    api.fund(Chain::Bitcoin, &address(&key), &[100_000]);
    api.state.lock().unwrap().second_opinion = Some(90_000);
    w.refresh();
    assert!(w.view().bitcoin_disagrees.is_none(), "off unless turned on");
    let view = w.set_check_balances(true);
    assert_eq!(view.bitcoin_disagrees.as_deref(), Some("0.0009"));
    api.state.lock().unwrap().second_opinion = None;
    w.refresh();
    assert!(w.view().bitcoin_disagrees.is_none(), "they agree");
}
