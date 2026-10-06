//! Dogecoin and DogecoinVM. The core must reproduce DogecoinVM's web wallet
//! vectors byte for byte. They come from dogecoin-vm
//! (`cmd/dogevm/testdata/wallet-vectors.json`, taken at commit 8e50b1f),
//! where btcd's script engine and the Go bridge code verify them; copy a new
//! version here with `scripts/sync-vectors.sh`.

mod common;

use btcvm_wallet_core::bridge::{
    self, CheckSource, DogeBridgeInfo, Pinned, Signers, doge_mainnet, peg_out_data_for,
    peg_out_destination_for,
};
use btcvm_wallet_core::doge::{FEE_PER_BYTE, HARD_DUST, SOFT_DUST, VM_FEE_PER_BYTE};
use btcvm_wallet_core::payment::describe_outputs;
use btcvm_wallet_core::tx::parse_tx;
use btcvm_wallet_core::wallet::{Coins, paused_by_signer_change};
use btcvm_wallet_core::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vectors {
    versions: DogeVersions,
    keys: Vec<KeyVector>,
    deposit: DepositVector,
    reserve_script: String,
    payments: Vec<PaymentVector>,
}

#[derive(Deserialize)]
struct DogeVersions {
    p2pkh: u8,
    p2sh: u8,
    wif: u8,
}

#[derive(Deserialize)]
struct KeyVector {
    label: String,
    hash160: String,
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
    hash160: String,
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
    /// Set for DogecoinVM payments, which pay its relay minimum.
    #[serde(default)]
    fee_per_byte: Option<String>,
}

impl PaymentVector {
    fn raw_txs(&self) -> HashMap<String, String> {
        self.utxos
            .iter()
            .map(|u| (u.txid.clone(), self.prev_tx.clone()))
            .collect()
    }

    fn request(&self) -> Request {
        let chain = match self.fee_per_byte.as_deref() {
            None => Chain::Dogecoin,
            Some(rate) => {
                assert_eq!(rate.parse::<u64>().unwrap(), VM_FEE_PER_BYTE);
                Chain::Dogecoinvm
            }
        };
        Request {
            chain,
            to: Destination::from_script(&hex::decode(&self.to_script).unwrap())
                .expect("vectors pay standard scripts"),
            amount: self.amount.parse().unwrap(),
            data: (!self.data.is_empty()).then(|| hex::decode(&self.data).unwrap()),
            fee_rate: 0,
        }
    }
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!("vectors/doge-wallet-vectors.json")).expect("vectors parse")
}

fn key_for(label: &str) -> Key {
    Key::from_bytes(&Sha256::digest(label.as_bytes())).unwrap()
}

fn payment<'a>(v: &'a Vectors, name: &str) -> &'a PaymentVector {
    v.payments.iter().find(|p| p.name == name).unwrap()
}

#[test]
fn vectors_are_for_dogecoin_mainnet() {
    let v = vectors().versions;
    assert_eq!(
        (v.p2pkh, v.p2sh, v.wif),
        (DOGE_MAINNET.p2pkh, DOGE_MAINNET.p2sh, DOGE_MAINNET.wif)
    );
    assert_eq!(Coin::Doge.network(), DOGE_MAINNET);
}

#[test]
fn keys_and_addresses() {
    for k in &vectors().keys {
        let key = key_for(&k.label);
        let dest = key.destination_for(Coin::Doge);
        assert_eq!(dest.kind(), Kind::P2pkh);
        assert_eq!(hex::encode(dest.program()), k.hash160, "{}", k.label);
        assert_eq!(dest.address(&DOGE_MAINNET), k.address, "{}", k.label);
        assert_eq!(key.wif(&DOGE_MAINNET).as_str(), k.wif, "{}", k.label);
        // The `Q…` WIF parses back to the same key, as a Bitcoin one does,
        // and the `D…` address to the same destination.
        assert_eq!(Key::parse(&k.wif).unwrap().bytes(), key.bytes());
        assert_eq!(decode_address(&k.address, &DOGE_MAINNET).unwrap(), dest);
        // The same key's Bitcoin address is another destination.
        assert_ne!(key.destination_for(Coin::Btc), dest);
    }
}

#[test]
fn addresses_of_other_networks_are_refused() {
    let doge = &vectors().keys[0].address;
    assert!(decode_address(doge, &MAINNET).is_err());
    for btc in [
        "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
        "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2",
        "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
    ] {
        let err = decode_address(btc, &DOGE_MAINNET).unwrap_err();
        assert!(err.message().contains("different network"), "{btc}: {err}");
    }
}

#[test]
fn deposit_and_peg_scripts() {
    let v = vectors();
    let d = &v.deposit;
    let dest = Destination::new(
        Kind::from_byte(d.dest.kind).unwrap(),
        &hex::decode(&d.dest.hash160).unwrap(),
    )
    .unwrap();
    // The deposit script is the same bytes as BTCVM's; Dogecoin locks to it
    // with P2SH.
    assert_eq!(
        hex::encode(d.signers.deposit_script(&dest).unwrap()),
        d.redeem_script
    );
    let deposit = d
        .signers
        .deposit_destination_for(Coin::Doge, &dest)
        .unwrap();
    assert_eq!(deposit.kind(), Kind::P2sh);
    assert_eq!(deposit.address(&DOGE_MAINNET), d.address);
    assert_eq!(decode_address(&d.address, &DOGE_MAINNET).unwrap(), deposit);
    // The reserve, and the peg on Dogecoin: P2SH of the signers' multisig.
    assert_eq!(
        hex::encode(d.signers.peg_for(Coin::Doge).unwrap().pk_script()),
        v.reserve_script
    );
}

#[test]
fn payments_match_byte_for_byte() {
    let v = vectors();
    for p in &v.payments {
        let key = key_for(&p.from_label);
        let req = p.request();
        let plan = plan_payment(
            &key.destination_for(Coin::Doge),
            &p.utxos,
            &p.raw_txs(),
            &req,
        )
        .unwrap_or_else(|e| panic!("{}: {e}", p.name));
        assert_eq!(plan.fee.to_string(), p.fee, "{}: fee", p.name);
        let signed = sign_plan(&key, &plan).unwrap();
        assert_eq!(signed.hex, p.tx, "{}: transaction", p.name);
        assert_eq!(signed.txid, p.txid, "{}: txid", p.name);
        // Planning and signing in one step gives the same bytes.
        let built = build_payment(&key, &p.utxos, &p.raw_txs(), &req).unwrap();
        assert_eq!(built.hex, p.tx, "{}: built", p.name);
        // A legacy transaction: version 1, final inputs, no witnesses.
        let parsed = parse_tx(&hex::decode(&signed.hex).unwrap()).unwrap();
        assert_eq!(parsed.version, 1);
        assert!(parsed.sequences.iter().all(|&s| s == 0xffff_ffff));
        assert!(parsed.witnesses.iter().all(Vec::is_empty));
    }
}

#[test]
fn fees_follow_the_chain() {
    let v = vectors();
    let one = payment(&v, "one input with change");
    assert_eq!(one.fee.parse::<u64>().unwrap(), 227 * FEE_PER_BYTE);
    let vm = payment(&v, "DogecoinVM payment at the relay minimum");
    assert_eq!(vm.fee.parse::<u64>().unwrap(), 376 * VM_FEE_PER_BYTE);
    // A payment below the soft dust limit pays that limit again.
    let small = payment(&v, "below the soft dust limit pays the surcharge");
    assert_eq!(
        small.fee.parse::<u64>().unwrap(),
        227 * FEE_PER_BYTE + SOFT_DUST
    );
    // Change below it goes to the fee.
    let tiny = payment(&v, "change below the soft dust limit goes to the fee");
    assert_eq!(tiny.fee.parse::<u64>().unwrap(), SOFT_DUST);
}

#[test]
fn a_withdrawal_tag_names_the_dogecoin_address() {
    let v = vectors();
    let to = key_for(&v.keys[2].label).destination_for(Coin::Doge);
    let p = payment(&v, "withdrawal with a DVMO tag");
    let data = hex::decode(&p.data).unwrap();
    assert_eq!(peg_out_data_for(Coin::Doge, &to), data);
    assert_eq!(peg_out_destination_for(Coin::Doge, &data), Some(to.clone()));
    // A BVMO tag isn't DogecoinVM's, nor a DVMO one BTCVM's.
    assert_eq!(peg_out_destination_for(Coin::Btc, &data), None);
    let bvmo = peg_out_data_for(Coin::Btc, &to);
    assert_eq!(peg_out_destination_for(Coin::Doge, &bvmo), None);
    // DogecoinVM's bridge pays only P2PKH and P2SH.
    let segwit = Destination::new(Kind::P2wpkh, &[7; 20]).unwrap();
    assert_eq!(
        peg_out_destination_for(Coin::Doge, &peg_out_data_for(Coin::Doge, &segwit)),
        None
    );

    // The review shows where the bridge will pay, and which output is change.
    let key = key_for(&p.from_label);
    let parsed = parse_tx(&hex::decode(&p.tx).unwrap()).unwrap();
    let view = describe_outputs(
        &parsed.outputs,
        &key.destination_for(Coin::Doge),
        &DOGE_MAINNET,
        Coin::Doge,
    );
    assert_eq!(
        view[1].withdrawal_to.as_deref(),
        Some(v.keys[2].address.as_str())
    );
    assert!(view.last().unwrap().change);
}

#[test]
fn the_smallest_payment_and_the_wallets_own_coins() {
    let key = key_for("dogevm vector key 1");
    let from = key.destination_for(Coin::Doge);
    let (utxos, raw) = common::coins(&from, &[100_000_000]);
    let to = key_for("dogevm vector key 2").destination_for(Coin::Doge);
    let req = |amount| Request {
        chain: Chain::Dogecoin,
        to: to.clone(),
        amount,
        data: None,
        fee_rate: 0,
    };
    assert!(plan_payment(&from, &utxos, &raw, &req(HARD_DUST - 1)).is_err());
    assert!(plan_payment(&from, &utxos, &raw, &req(HARD_DUST)).is_ok());
    // DOGE is spent from the P2PKH address only, never from the SegWit one.
    let segwit = key.destination_for(Coin::Btc);
    assert!(plan_payment(&segwit, &utxos, &raw, &req(SOFT_DUST)).is_err());
    // And a plan signs only with its own key.
    let plan = plan_payment(&from, &utxos, &raw, &req(SOFT_DUST)).unwrap();
    assert!(sign_plan(&key_for("dogevm vector key 2"), &plan).is_err());
}

#[test]
fn max_spends_every_coin() {
    let key = key_for("dogevm vector key 1");
    let from = key.destination_for(Coin::Doge);
    let (utxos, raw) = common::coins(&from, &[300_000_000, 500_000_000, 700_000_000]);
    let to = key_for("dogevm vector key 2").destination_for(Coin::Doge);
    for chain in [Chain::Dogecoin, Chain::Dogecoinvm] {
        let mut req = Request {
            chain,
            to: to.clone(),
            amount: 0,
            data: None,
            fee_rate: 0,
        };
        let max = max_payment(&from, &utxos, &raw, &req).unwrap();
        req.amount = max;
        let plan = plan_payment(&from, &utxos, &raw, &req).unwrap();
        assert_eq!(plan.total_in, 1_500_000_000, "{chain:?}");
        assert_eq!(plan.outputs().len(), 1, "{chain:?}: no change");
        assert_eq!(max + plan.fee, plan.total_in, "{chain:?}");
        req.amount = max + 1;
        assert!(
            plan_payment(&from, &utxos, &raw, &req).is_err(),
            "{chain:?}"
        );
    }
}

#[test]
fn a_phrase_makes_the_standard_dogecoin_address() {
    // BIP 39's "abandon … about" at m/44'/3'/0'/0/0, worked out apart from
    // this code (pure Python: PBKDF2, BIP 32, secp256k1) and the address
    // Dogecoin wallets that take BIP 39 phrases show for it.
    let entropy = seed::parse_words(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .unwrap();
    let doge = seed::key_for_coin(&entropy, Coin::Doge).unwrap();
    assert_eq!(
        doge.destination_for(Coin::Doge).address(&DOGE_MAINNET),
        "DBus3bamQjgJULBJtYXpEzDWQRwF5iwxgC"
    );
    // BTC keeps BIP 84's key: the phrase's Bitcoin address doesn't move.
    let btc = seed::key_for_coin(&entropy, Coin::Btc).unwrap();
    assert_eq!(
        btc.destination().address(&MAINNET),
        "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"
    );
    assert_eq!(
        seed::key_from_entropy(&entropy).unwrap().bytes(),
        btc.bytes()
    );
}

/// metaldoge.com's `/api/info`, saved on 2026-10-06.
fn live_info() -> DogeBridgeInfo {
    serde_json::from_str(include_str!("fixtures/metaldoge-info-2026-10-06.json")).unwrap()
}

#[test]
fn pinned_keys_make_the_pinned_peg_address() {
    let pinned = Pinned::doge_mainnet();
    assert_eq!(pinned.coin, Coin::Doge);
    assert_eq!(
        pinned
            .signers
            .peg_for(Coin::Doge)
            .unwrap()
            .address(&DOGE_MAINNET),
        doge_mainnet::PEG_ADDRESS
    );
}

#[test]
fn the_live_doge_bridge_checks_out() {
    let b = bridge::verify_doge(&live_info(), &Pinned::doge_mainnet()).unwrap();
    assert_eq!(b.coin, Coin::Doge);
    assert_eq!(b.peg.address(&DOGE_MAINNET), doge_mainnet::PEG_ADDRESS);
    assert!(b.signer_change.is_none());
    assert_eq!(
        (b.min_deposit, b.max_deposit, b.min_peg_out),
        (100_000_000, 10_000_000_000, 200_000_000)
    );
    assert_eq!(b.fee_rate, FEE_PER_BYTE);
    // Each bridge's info is checked only against its own coin's pins.
    assert!(bridge::verify_doge(&live_info(), &Pinned::mainnet()).is_err());
}

#[test]
fn another_network_is_refused() {
    let mut info = live_info();
    info.dogecoin_network = "testnet".into();
    let err = bridge::verify_doge(&info, &Pinned::doge_mainnet()).unwrap_err();
    assert!(err.is_untrusted());
    let mut info = live_info();
    info.dogecoin_versions.p2pkh = 0;
    assert!(
        bridge::verify_doge(&info, &Pinned::doge_mainnet())
            .unwrap_err()
            .is_untrusted()
    );
    // A peg address that doesn't follow from the signers.
    let mut info = live_info();
    info.peg_address = vectors().deposit.address;
    assert!(
        bridge::verify_doge(&info, &Pinned::doge_mainnet())
            .unwrap_err()
            .is_untrusted()
    );
}

#[test]
fn a_new_signer_set_pauses_dogecoin_moves() {
    let mut info = live_info();
    let newcomer = Key::parse(&"22".repeat(32)).unwrap();
    info.signers.public_keys[0] = hex::encode(newcomer.public_key());
    let new_peg = info
        .signers
        .peg_for(Coin::Doge)
        .unwrap()
        .address(&DOGE_MAINNET);
    info.peg_address = new_peg.clone();
    info.reserve_address = new_peg.clone();

    let b = bridge::verify_doge(&info, &Pinned::doge_mainnet()).unwrap();
    let change = b.signer_change.clone().expect("the change is reported");
    assert_eq!(change.coin, Coin::Doge);
    let links = change.where_to_check(&DOGE_MAINNET);
    assert_eq!(links[0].source, CheckSource::OldPegOnDogecoin);
    assert!(links[0].url.ends_with(doge_mainnet::PEG_ADDRESS));
    assert!(links[0].url.contains("blockchair.com/dogecoin"));
    assert_eq!(links[1].source, CheckSource::NewPegOnDogecoin);
    assert!(links[1].url.ends_with(&new_peg));

    // Moves between Dogecoin and DogecoinVM stop; the message says which
    // chains.
    let key = key_for("dogevm vector key 1");
    let from = key.destination_for(Coin::Doge);
    let (utxos, raw) = common::coins(&from, &[5_000_000_000]);
    let coins = Coins {
        utxos: &utxos,
        raw_txs: &raw,
    };
    let err = wallet::plan_withdrawal(
        &b,
        &from,
        coins,
        300_000_000,
        &key_for("x")
            .destination_for(Coin::Doge)
            .address(&DOGE_MAINNET),
    )
    .unwrap_err();
    assert!(err.is_untrusted());
    assert_eq!(err.message(), paused_by_signer_change(Coin::Doge));
    assert!(err.message().contains("Dogecoin and DogecoinVM"));
}

#[test]
fn deposits_withdrawals_and_sends() {
    let b = bridge::verify_doge(&live_info(), &Pinned::doge_mainnet()).unwrap();
    let key = key_for("dogevm vector key 1");
    let from = key.destination_for(Coin::Doge);
    let (utxos, raw) = common::coins(&from, &[5_000_000_000]);
    let coins = Coins {
        utxos: &utxos,
        raw_txs: &raw,
    };

    // A deposit pays the personal deposit address the pinned signers make,
    // and only if the bridge gave that one.
    let deposit = b
        .signers
        .deposit_destination_for(Coin::Doge, &from)
        .unwrap()
        .address(&DOGE_MAINNET);
    let plan = wallet::plan_deposit(&b, &from, coins, 1_000_000_000, &deposit).unwrap();
    assert_eq!(plan.chain, Chain::Dogecoin);
    assert_eq!(
        plan.describe(&DOGE_MAINNET)[0].address.as_deref(),
        Some(deposit.as_str())
    );
    let wrong = vectors().deposit.address;
    assert!(
        wallet::plan_deposit(&b, &from, coins, 1_000_000_000, &wrong)
            .unwrap_err()
            .is_untrusted()
    );
    let err = wallet::plan_deposit(&b, &from, coins, 50_000_000, &deposit).unwrap_err();
    assert_eq!(
        err.message(),
        "the smallest deposit is 1 DOGE; a smaller one is not credited"
    );
    let err = wallet::plan_deposit(&b, &from, coins, 10_100_000_000, &deposit).unwrap_err();
    assert!(
        err.message()
            .starts_with("a deposit can be at most 100 DOGE")
    );

    // A withdrawal pays the reserve on DogecoinVM, tagged with the Dogecoin
    // address to pay.
    let to = key_for("dogevm vector key 2").destination_for(Coin::Doge);
    let plan =
        wallet::plan_withdrawal(&b, &from, coins, 300_000_000, &to.address(&DOGE_MAINNET)).unwrap();
    assert_eq!(plan.chain, Chain::Dogecoinvm);
    let view = plan.describe(&DOGE_MAINNET);
    assert_eq!(view[0].address.as_deref(), Some(doge_mainnet::PEG_ADDRESS));
    assert_eq!(
        view[1].withdrawal_to.as_deref(),
        Some(to.address(&DOGE_MAINNET).as_str())
    );
    assert!(
        wallet::plan_withdrawal(&b, &from, coins, 300_000_000, doge_mainnet::PEG_ADDRESS).is_err()
    );
    let err = wallet::plan_withdrawal(&b, &from, coins, 100_000_000, &to.address(&DOGE_MAINNET))
        .unwrap_err();
    assert_eq!(
        err.message(),
        "the smallest withdrawal is 2 DOGE; a smaller one is not paid"
    );

    // Sends: on either chain, never to the peg itself, never on BTC's.
    let plan = wallet::plan_send(
        &b,
        Chain::Dogecoinvm,
        &from,
        coins,
        &to.address(&DOGE_MAINNET),
        100_000_000,
    )
    .unwrap();
    assert_eq!(plan.chain, Chain::Dogecoinvm);
    let err = wallet::plan_send(
        &b,
        Chain::Dogecoin,
        &from,
        coins,
        doge_mainnet::PEG_ADDRESS,
        100_000_000,
    )
    .unwrap_err();
    assert!(err.message().contains("Move to DogecoinVM"), "{err}");
    let err = wallet::plan_send(
        &b,
        Chain::Dogecoinvm,
        &from,
        coins,
        doge_mainnet::PEG_ADDRESS,
        100_000_000,
    )
    .unwrap_err();
    assert!(err.message().contains("Withdraw to Dogecoin"), "{err}");
    assert!(
        wallet::plan_send(
            &b,
            Chain::Bitcoin,
            &from,
            coins,
            &to.address(&DOGE_MAINNET),
            100_000_000
        )
        .is_err()
    );
    assert!(
        wallet::plan_send(
            &b,
            Chain::Dogecoin,
            &from,
            coins,
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            100_000_000
        )
        .is_err()
    );
}

#[test]
fn chains_and_coins() {
    for chain in Chain::ALL {
        let coin = chain.coin();
        assert!(chain == coin.l1() || chain == coin.vm());
        assert_eq!(chain.is_vm(), chain == coin.vm());
    }
    assert_eq!(Coin::Doge.ticker(), "DOGE");
    assert_eq!(Coin::Doge.parse_amount("1,5").ok(), None);
    assert_eq!(Coin::Doge.parse_amount("4200").unwrap(), 420_000_000_000);
    assert_eq!(Coin::Btc.parse_amount("4200").unwrap(), 420_000_000_000);
    // BTC's amounts stay within Bitcoin's supply, DOGE's within Dogecoin's.
    assert!(Coin::Btc.parse_amount("100000000").is_err());
    assert_eq!(
        Coin::Doge.parse_amount("100000000").unwrap(),
        10_000_000_000_000_000
    );
    assert_eq!(
        serde_json::to_string(&Chain::Dogecoinvm).unwrap(),
        "\"dogecoinvm\""
    );
}
