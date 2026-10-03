//! The bridge check: the signer set pinned for mainnet, and what happens
//! when the bridge's `/api/info` says something else.

use btcvm_wallet_core::bridge::{self, BridgeInfo, Pinned, mainnet};
use btcvm_wallet_core::*;

/// metalbtc.com's `/api/info`, saved on 2026-10-03.
fn live_info() -> BridgeInfo {
    serde_json::from_str(include_str!("fixtures/metalbtc-info-2026-10-03.json")).unwrap()
}

#[test]
fn pinned_keys_make_the_pinned_peg_address() {
    let pinned = Pinned::mainnet();
    assert_eq!(
        pinned.signers.peg().unwrap().address(&MAINNET),
        mainnet::PEG_ADDRESS
    );
}

#[test]
fn the_live_bridge_checks_out() {
    let b = bridge::verify(&live_info(), &Pinned::mainnet()).unwrap();
    assert_eq!(b.peg.address(&MAINNET), mainnet::PEG_ADDRESS);
    assert_eq!(
        (b.min_deposit, b.max_deposit, b.min_peg_out),
        (10_000, 100_000, 5_000)
    );
    assert_eq!(b.btc_fee_rate, 2);
}

#[test]
fn a_bridge_that_says_otherwise_is_refused() {
    let pinned = Pinned::mainnet();
    let refused = |change: &dyn Fn(&mut BridgeInfo)| {
        let mut info = live_info();
        change(&mut info);
        bridge::verify(&info, &pinned).unwrap_err()
    };

    // Its own keys in place of the signers', consistently: the peg and
    // reserve addresses made from them.
    let attacker = Key::parse(&"11".repeat(32)).unwrap();
    let e = refused(&|i| {
        i.signers.public_keys[0] = hex::encode(attacker.public_key());
        let peg = i.signers.peg().unwrap().address(&MAINNET);
        i.peg_address = peg.clone();
        i.reserve_address = peg;
    });
    assert!(
        e.is_untrusted() && e.message().contains("signer set"),
        "{e}"
    );

    // The same keys in another order, or a lower threshold, are a
    // different peg.
    assert!(refused(&|i| i.signers.public_keys.swap(0, 1)).is_untrusted());
    assert!(refused(&|i| i.signers.required = 1).is_untrusted());

    // A reserve or peg address that isn't the signers'.
    let elsewhere = attacker.destination().address(&MAINNET);
    assert!(
        refused(&|i| i.reserve_address = elsewhere.clone())
            .message()
            .contains("reserve")
    );
    assert!(
        refused(&|i| i.peg_address = elsewhere.clone())
            .message()
            .contains("peg")
    );

    // Another chain or network, or another network's address formats.
    assert!(
        refused(&|i| i.chain_id = "2q9e4r6Mu3U68nU1fYjgbR6JvwrRx36CohpAX5UQxse55x1Q5".into())
            .is_untrusted()
    );
    assert!(refused(&|i| i.bitcoin_network = "testnet3".into()).is_untrusted());
    assert!(refused(&|i| i.btcvm_versions.hrp = "tb".into()).is_untrusted());

    // A fee rate or limits that make no sense.
    assert!(
        refused(&|i| i.btc_fee_rate = 5_000)
            .message()
            .contains("fee rate")
    );
    assert!(refused(&|i| i.btc_fee_rate = 0).is_untrusted());
    assert!(
        refused(&|i| i.min_deposit = "0.01".into())
            .message()
            .contains("limits")
    );
    assert!(refused(&|i| i.max_deposit = "lots".into()).is_untrusted());
}

#[test]
fn signer_sets_must_be_sound() {
    let mut s = Pinned::mainnet().signers;
    s.public_keys[2] = s.public_keys[0].clone();
    assert!(s.witness_script().unwrap_err().message().contains("twice"));

    let mut s = Pinned::mainnet().signers;
    s.required = 4;
    assert!(s.witness_script().is_err());
    s.required = 0;
    assert!(s.witness_script().is_err());

    // Not a point on the curve.
    let mut s = Pinned::mainnet().signers;
    s.public_keys[1] = format!("02{}", "00".repeat(32));
    assert!(s.witness_script().is_err());
}

#[test]
fn deposit_addresses_follow_from_the_pinned_signers() {
    let b = bridge::verify(&live_info(), &Pinned::mainnet()).unwrap();
    let mine = Key::parse(&"22".repeat(32)).unwrap().destination();
    let theirs = Key::parse(&"33".repeat(32)).unwrap().destination();
    let a = b.signers.deposit_destination(&mine).unwrap();
    // A P2WSH address on Bitcoin, unique to each BTCVM address.
    assert_eq!(a.kind(), Kind::P2wsh);
    assert!(a.address(&MAINNET).starts_with("bc1q"));
    assert_ne!(a, b.signers.deposit_destination(&theirs).unwrap());
    assert_ne!(a, b.peg);
}
