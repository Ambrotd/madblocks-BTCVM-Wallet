//! The core must reproduce the BTCVM web wallet's vectors byte for byte. They
//! come from btc-vm (`cmd/btcvm/testdata/wallet-vectors.json`, taken at
//! commit 42bc2a2), where btcd's script engine and the Go bridge code verify
//! them; copy a new version here with `scripts/sync-vectors.sh`.

use btcvm_wallet_core::bridge::{self, BridgeInfo, Pinned, Versions, peg_out_data};
use btcvm_wallet_core::payment::{BTC_FEE_RATE, describe_outputs};
use btcvm_wallet_core::tx::{self, parse_tx};
use btcvm_wallet_core::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vectors {
    versions: Versions,
    keys: Vec<KeyVector>,
    deposit: DepositVector,
    reserve_script: String,
    payments: Vec<PaymentVector>,
}

#[derive(Deserialize)]
struct KeyVector {
    label: String,
    program: String,
    address: String,
    wif: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DepositVector {
    signers: Signers,
    dest: DestVector,
    redeem_script: String,
    address: String,
}

#[derive(Deserialize)]
struct DestVector {
    kind: u8,
    program: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymentVector {
    name: String,
    from_label: String,
    prev_tx: String,
    utxos: Vec<Utxo>,
    to_script: String,
    amount: String,
    data: String,
    tx: String,
    txid: String,
    fee: String,
    fee_rate: String,
}

impl PaymentVector {
    fn raw_txs(&self) -> HashMap<String, String> {
        self.utxos
            .iter()
            .map(|u| (u.txid.clone(), self.prev_tx.clone()))
            .collect()
    }

    fn request(&self) -> Request {
        let (chain, fee_rate) = match self.fee_rate.as_str() {
            "vm" => (Chain::Btcvm, BTC_FEE_RATE),
            rate => (Chain::Bitcoin, rate.parse().unwrap()),
        };
        Request {
            chain,
            to: Destination::from_script(&hex::decode(&self.to_script).unwrap())
                .expect("vectors pay standard scripts"),
            amount: self.amount.parse().unwrap(),
            data: (!self.data.is_empty()).then(|| hex::decode(&self.data).unwrap()),
            fee_rate,
        }
    }
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!("vectors/wallet-vectors.json")).expect("vectors parse")
}

fn key_for(label: &str) -> Key {
    Key::from_bytes(&Sha256::digest(label.as_bytes())).unwrap()
}

fn payment<'a>(v: &'a Vectors, name: &str) -> &'a PaymentVector {
    v.payments.iter().find(|p| p.name == name).unwrap()
}

#[test]
fn vectors_are_for_mainnet() {
    let v = vectors().versions;
    assert_eq!(
        (v.hrp.as_str(), v.p2pkh, v.p2sh, v.wif),
        (MAINNET.hrp, MAINNET.p2pkh, MAINNET.p2sh, MAINNET.wif)
    );
}

#[test]
fn keys_and_addresses() {
    for k in &vectors().keys {
        let key = key_for(&k.label);
        let dest = key.destination();
        assert_eq!(hex::encode(dest.program()), k.program, "{}", k.label);
        assert_eq!(dest.address(&MAINNET), k.address, "{}", k.label);
        assert_eq!(key.wif(&MAINNET).as_str(), k.wif, "{}", k.label);
        // The WIF parses back to the same key, and the address to the same
        // destination.
        assert_eq!(Key::parse(&k.wif).unwrap().bytes(), key.bytes());
        assert_eq!(decode_address(&k.address, &MAINNET).unwrap(), dest);
    }
}

#[test]
fn deposit_and_peg_scripts() {
    let v = vectors();
    let d = &v.deposit;
    let dest = Destination::new(
        Kind::from_byte(d.dest.kind).unwrap(),
        &hex::decode(&d.dest.program).unwrap(),
    )
    .unwrap();
    assert_eq!(
        hex::encode(d.signers.deposit_script(&dest).unwrap()),
        d.redeem_script
    );
    let deposit = d.signers.deposit_destination(&dest).unwrap();
    assert_eq!(deposit.address(&MAINNET), d.address);
    assert_eq!(decode_address(&d.address, &MAINNET).unwrap(), deposit);
    assert_eq!(
        hex::encode(d.signers.peg().unwrap().pk_script()),
        v.reserve_script
    );
}

#[test]
fn payments_match_byte_for_byte() {
    let v = vectors();
    for p in &v.payments {
        let key = key_for(&p.from_label);
        let req = p.request();
        let plan = plan_payment(&key.destination(), &p.utxos, &p.raw_txs(), &req)
            .unwrap_or_else(|e| panic!("{}: {e}", p.name));
        assert_eq!(plan.fee.to_string(), p.fee, "{}", p.name);
        let signed = sign_plan(&key, &plan).unwrap();
        assert_eq!(signed.hex, p.tx, "{}", p.name);
        assert_eq!(signed.txid, p.txid, "{}", p.name);
        assert_eq!(signed.fee.to_string(), p.fee, "{}", p.name);

        // Read back: the id is of the transaction without its witnesses, it
        // spends only coins it was given, and it pays what the review showed.
        let parsed = parse_tx(&hex::decode(&signed.hex).unwrap()).unwrap();
        assert_eq!(parsed.txid(), p.txid, "{}", p.name);
        for (id, vout) in &parsed.inputs {
            assert!(
                p.utxos.iter().any(|u| &u.txid == id && u.vout == *vout),
                "{}: {id}:{vout}",
                p.name
            );
        }
        assert_eq!(
            plan.describe(&MAINNET),
            describe_outputs(&parsed.outputs, &key.destination(), &MAINNET),
            "{}: the review shows exactly what is signed",
            p.name
        );
        // The unsigned transaction is the signed one without witnesses.
        assert_eq!(
            parse_tx(&hex::decode(plan.unsigned_hex()).unwrap())
                .unwrap()
                .txid(),
            p.txid
        );
    }
}

#[test]
fn review_names_a_withdrawals_destination() {
    let v = vectors();
    let p = payment(&v, "BTCVM withdrawal");
    let key = key_for(&p.from_label);
    let plan = plan_payment(&key.destination(), &p.utxos, &p.raw_txs(), &p.request()).unwrap();
    let outputs = plan.describe(&MAINNET);
    assert_eq!(
        outputs[0].address.as_deref(),
        Some(v.deposit.signers.peg().unwrap().address(&MAINNET).as_str())
    );
    assert_eq!(
        outputs[1].withdrawal_to.as_deref(),
        Some(v.keys[2].address.as_str())
    );
    assert!(outputs[2].change);
}

#[test]
fn a_lying_server_changes_nothing() {
    let v = vectors();
    let p = &v.payments[0];
    let key = key_for(&p.from_label);

    // A coin's claimed value is ignored: the verified transaction decides.
    let mut inflated = p.utxos.clone();
    inflated[0].value = "999999999999999".into();
    let got = build_payment(&key, &inflated, &p.raw_txs(), &p.request()).unwrap();
    assert_eq!(got.hex, p.tx);

    // Another transaction in place of the coin's own is caught.
    let wrong: HashMap<String, String> = p
        .utxos
        .iter()
        .map(|u| (u.txid.clone(), v.payments[1].prev_tx.clone()))
        .collect();
    let e = build_payment(&key, &p.utxos, &wrong, &p.request()).unwrap_err();
    assert!(
        e.is_untrusted() && e.message().contains("wrong transaction"),
        "{e}"
    );

    // A coin listed as the wallet's whose transaction pays someone else is
    // caught.
    let other = key_for(&v.keys[1].label);
    let raw = prev_tx(1, &[(500_000_000, other.destination().pk_script())]);
    let mut listed = p.utxos.clone();
    listed[0].txid = tx::txid(&raw).unwrap();
    let raw_txs = HashMap::from([(listed[0].txid.clone(), hex::encode(&raw))]);
    let e = build_payment(&key, &listed, &raw_txs, &p.request()).unwrap_err();
    assert!(e.is_untrusted() && e.message().contains("not yours"), "{e}");

    // A plan can only be signed by the key whose coins it spends.
    let plan = plan_payment(&key.destination(), &p.utxos, &p.raw_txs(), &p.request()).unwrap();
    assert!(sign_plan(&other, &plan).is_err());
}

/// A minimal legacy transaction paying `outputs`, for coins to spend, as
/// the vectors' generator makes them.
fn prev_tx(tag: u8, outputs: &[(u64, Vec<u8>)]) -> Vec<u8> {
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

#[test]
fn fees_follow_chain_js_for_many_coins() {
    // What chain.js charges, at 1 sat/vB, for a payment that needs 1 to 5
    // coins of 100,000 sats (run through the web wallet's planPayment).
    let key = key_for("btcvm vector key 1");
    let to = key_for("btcvm vector key 2").destination();
    for (n, want) in [(1u64, 141), (2, 209), (3, 278), (4, 346), (5, 414)] {
        let outputs = vec![(100_000, key.destination().pk_script()); n as usize];
        let raw = prev_tx(n as u8, &outputs);
        let txid = tx::txid(&raw).unwrap();
        let utxos: Vec<Utxo> = (0..n as u32)
            .map(|vout| Utxo {
                txid: txid.clone(),
                vout,
                value: "100000".into(),
                script: hex::encode(key.destination().pk_script()),
                confirmations: 1,
            })
            .collect();
        let raw_txs = HashMap::from([(txid, hex::encode(&raw))]);
        let req = Request {
            chain: Chain::Bitcoin,
            to: to.clone(),
            amount: n * 100_000 - 2_000,
            data: None,
            fee_rate: 1,
        };
        let plan = plan_payment(&key.destination(), &utxos, &raw_txs, &req).unwrap();
        assert_eq!(
            (plan.spends().len() as u64, plan.fee),
            (n, want),
            "{n} coins"
        );
    }
}

#[test]
fn fees_are_capped() {
    let v = vectors();
    let p = payment(&v, "at a high fee rate");
    let key = key_for(&p.from_label);
    let mut req = p.request();
    req.fee_rate = 5_000;
    let e = plan_payment(&key.destination(), &p.utxos, &p.raw_txs(), &req).unwrap_err();
    assert!(e.message().contains("looks wrong"), "{e}");
    req.fee_rate = 0;
    assert!(plan_payment(&key.destination(), &p.utxos, &p.raw_txs(), &req).is_err());
    // The highest rate allowed: 141 vB at 1,000 sat/vB is under the cap.
    req.fee_rate = 1_000;
    assert_eq!(
        plan_payment(&key.destination(), &p.utxos, &p.raw_txs(), &req)
            .unwrap()
            .fee,
        141_000
    );

    // Three coins at that rate cost 278,000 sats, over the 250,000 cap.
    let p = payment(&v, "several inputs, largest first");
    let mut req = p.request();
    req.amount = 10_500_000;
    req.fee_rate = 1_000;
    let e = plan_payment(&key.destination(), &p.utxos, &p.raw_txs(), &req).unwrap_err();
    assert!(e.message().contains("refusing to sign"), "{e}");
}

/// A bridge pinned to the vectors' signer set, with limits that let the
/// vectors' amounts through.
fn vector_bridge(v: &Vectors) -> VerifiedBridge {
    let signers = v.deposit.signers.clone();
    let peg = signers.peg().unwrap().address(&MAINNET);
    let pinned = Pinned {
        net: MAINNET,
        bitcoin_network: "mainnet",
        btcvm_network: "btcvm",
        chain_id: "test",
        signers: signers.clone(),
    };
    let versions = || Versions {
        hrp: "bc".into(),
        p2pkh: 0,
        p2sh: 5,
        wif: 128,
    };
    let info = BridgeInfo {
        bitcoin_network: "mainnet".into(),
        btcvm_network: "btcvm".into(),
        bitcoin_versions: versions(),
        btcvm_versions: versions(),
        chain_id: "test".into(),
        peg_address: peg.clone(),
        reserve_address: peg,
        signers,
        btc_fee_rate: BTC_FEE_RATE,
        min_deposit: "0.0001".into(),
        max_deposit: "1".into(),
        min_peg_out: "0.00005".into(),
        max_circulating: "10".into(),
        deposit_confirmations: 6,
        confirmation_tiers: Vec::new(),
        btc_wallet: true,
    };
    bridge::verify(&info, &pinned).unwrap()
}

#[test]
fn deposits_and_withdrawals_reproduce_the_vectors() {
    let v = vectors();
    let bridge = vector_bridge(&v);
    let key = key_for(&v.keys[0].label);
    let from = key.destination();

    let p = payment(&v, "deposit to a personal deposit address");
    let raw = p.raw_txs();
    let coins = Coins {
        utxos: &p.utxos,
        raw_txs: &raw,
    };
    let plan = plan_deposit(
        &bridge,
        &from,
        coins,
        p.amount.parse().unwrap(),
        &v.deposit.address,
    )
    .unwrap();
    assert_eq!(sign_plan(&key, &plan).unwrap().hex, p.tx);

    // A deposit address the signers don't make is refused, whoever's it is.
    let e = plan_deposit(
        &bridge,
        &from,
        coins,
        p.amount.parse().unwrap(),
        &v.keys[1].address,
    )
    .unwrap_err();
    assert!(e.is_untrusted(), "{e}");
    let other = key_for(&v.keys[1].label).destination();
    let theirs = bridge
        .signers
        .deposit_destination(&other)
        .unwrap()
        .address(&MAINNET);
    assert!(
        plan_deposit(&bridge, &from, coins, p.amount.parse().unwrap(), &theirs)
            .unwrap_err()
            .is_untrusted()
    );

    // Outside the bridge's limits, nothing is planned.
    assert!(plan_deposit(&bridge, &from, coins, 9_999, &v.deposit.address).is_err());
    assert!(plan_deposit(&bridge, &from, coins, 100_000_001, &v.deposit.address).is_err());

    let p = payment(&v, "BTCVM withdrawal");
    let raw = p.raw_txs();
    let coins = Coins {
        utxos: &p.utxos,
        raw_txs: &raw,
    };
    let plan = plan_withdrawal(
        &bridge,
        &from,
        coins,
        p.amount.parse().unwrap(),
        &v.keys[2].address,
    )
    .unwrap();
    assert_eq!(sign_plan(&key, &plan).unwrap().hex, p.tx);
    assert_eq!(
        p.data,
        hex::encode(peg_out_data(
            &decode_address(&v.keys[2].address, &MAINNET).unwrap()
        ))
    );
    assert!(plan_withdrawal(&bridge, &from, coins, 4_999, &v.keys[2].address).is_err());
}

#[test]
fn sending_to_the_peg_is_refused() {
    let v = vectors();
    let bridge = vector_bridge(&v);
    let key = key_for(&v.keys[0].label);
    let p = payment(&v, "one input with change");
    let raw = p.raw_txs();
    let coins = Coins {
        utxos: &p.utxos,
        raw_txs: &raw,
    };
    let peg = bridge.peg.address(&MAINNET);
    for chain in [Chain::Bitcoin, Chain::Btcvm] {
        assert!(plan_send(&bridge, chain, &key.destination(), coins, &peg, 1_000_000).is_err());
    }
    assert!(plan_withdrawal(&bridge, &key.destination(), coins, 1_000_000, &peg).is_err());
    // An ordinary send goes through, and pays exactly the vector.
    let plan = plan_send(
        &bridge,
        Chain::Bitcoin,
        &key.destination(),
        coins,
        &v.keys[1].address,
        120_000_000,
    )
    .unwrap();
    assert_eq!(sign_plan(&key, &plan).unwrap().hex, p.tx);
}

#[test]
fn keys_and_amounts_refuse_bad_input() {
    // The Bitcoin wiki's example WIF (uncompressed), public for years. Split
    // so key scanners don't take it for a leaked key.
    let textbook = concat!("5HueCGU8rMjxEXxiPuD5", "BDku4MkFqeZyd4dZ1jvhTVqvbTLvyTJ");
    assert!(Key::parse(textbook).is_err_and(|e| e.message().contains("uncompressed")));
    assert!(Key::parse(&"00".repeat(32)).is_err());
    assert!(
        Key::parse(&"ff".repeat(32)).is_err(),
        "above the curve order"
    );
    assert!(Key::parse("not a key").is_err());
    assert_eq!(format!("{:?}", key_for("x")), "Key(…)");

    assert_eq!(parse_btc("0.0025").unwrap(), 250_000);
    assert_eq!(parse_btc("0.00000001").unwrap(), 1);
    assert_eq!(parse_btc(" 21000000 ").unwrap(), 21_000_000 * SATS_PER_BTC);
    assert_eq!(
        parse_btc("0.00010000").unwrap(),
        10_000,
        "the bridge's format"
    );
    for bad in [
        "1.",
        ".5",
        "1.123456789",
        "-1",
        "21000000.00000001",
        "1,5",
        "",
        "1e3",
    ] {
        assert!(parse_btc(bad).is_err(), "{bad}");
    }
    assert_eq!(format_btc(250_000), "0.0025");
    assert_eq!(format_btc(SATS_PER_BTC), "1");

    let fresh = Key::generate();
    assert_eq!(
        Key::parse(&hex::encode(fresh.bytes())).unwrap().bytes(),
        fresh.bytes()
    );
}
