//! Recovery phrases against the BIPs' own vectors: BIP 39's English vectors
//! (python-mnemonic's, passphrase "TREZOR"), BIP 32's derivation vectors 1
//! to 4, and BIP 84's first address.

use btcvm_wallet_core::encoding::sha256d;
use btcvm_wallet_core::seed::{self, BIP84_FIRST};
use btcvm_wallet_core::{Key, MAINNET};
use serde_json::Value;

const HARDENED: u32 = 1 << 31;

/// An extended private key's chain code and key (BIP 32 serialization).
fn xprv_parts(xprv: &str) -> ([u8; 32], [u8; 32]) {
    let full = bs58::decode(xprv).into_vec().unwrap();
    let (raw, check) = full.split_at(full.len() - 4);
    assert_eq!(&sha256d(raw)[..4], check, "base58check");
    assert_eq!(raw.len(), 78);
    assert_eq!(&raw[..4], &[0x04, 0x88, 0xad, 0xe4], "an xprv");
    assert_eq!(raw[45], 0);
    (
        raw[13..45].try_into().unwrap(),
        raw[46..78].try_into().unwrap(),
    )
}

fn path(text: &str) -> Vec<u32> {
    text.split('/')
        .skip(1)
        .map(|p| match p.strip_suffix('\'') {
            Some(n) => n.parse::<u32>().unwrap() | HARDENED,
            None => p.parse().unwrap(),
        })
        .collect()
}

#[test]
fn bip39_english_vectors() {
    let v: Value = serde_json::from_str(include_str!("vectors/bip39-vectors.json")).unwrap();
    let passphrase = v["passphrase"].as_str().unwrap();
    let vectors = v["english"].as_array().unwrap();
    assert_eq!(vectors.len(), 24);
    for case in vectors {
        let entropy = hex::decode(case[0].as_str().unwrap()).unwrap();
        let phrase = case[1].as_str().unwrap();
        assert_eq!(&*seed::words(&entropy).unwrap(), phrase);
        assert_eq!(&**seed::parse_words(phrase).unwrap(), &entropy[..]);
        let s = seed::seed(phrase, passphrase);
        assert_eq!(hex::encode(&s[..]), case[2].as_str().unwrap(), "{phrase}");
        let (key, chain) = seed::extended_key(&s[..], &[]).unwrap();
        assert_eq!(xprv_parts(case[3].as_str().unwrap()), (*chain, *key));
    }
}

#[test]
fn bip32_derivation_vectors() {
    let v: Value = serde_json::from_str(include_str!("vectors/bip32-vectors.json")).unwrap();
    let mut chains = 0;
    for vector in v["vectors"].as_array().unwrap() {
        let s = hex::decode(vector["seed"].as_str().unwrap()).unwrap();
        for c in vector["chains"].as_array().unwrap() {
            let (key, chain) = seed::extended_key(&s, &path(c["path"].as_str().unwrap())).unwrap();
            assert_eq!(
                xprv_parts(c["xprv"].as_str().unwrap()),
                (*chain, *key),
                "{}",
                c["path"]
            );
            chains += 1;
        }
    }
    assert_eq!(chains, 17);
}

#[test]
fn bip84_first_receiving_address() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let entropy = seed::parse_words(phrase).unwrap();
    let key = seed::key_from_entropy(&entropy).unwrap();
    assert_eq!(
        &*key.wif(&MAINNET),
        "KyZpNDKnfs94vbrwhJneDi77V6jF64PWPF8x5cdJb8ifgg2DUc9d"
    );
    assert_eq!(
        hex::encode(key.public_key()),
        "0330d54fd0dd420a6e5f8d3624f5f3482cae350f79d5f0753bf5beef9c2d91af3c"
    );
    assert_eq!(
        key.destination().address(&MAINNET),
        "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"
    );
    assert_eq!(BIP84_FIRST, [84 | HARDENED, HARDENED, HARDENED, 0, 0]);
}

#[test]
fn phrases_are_read_forgivingly_but_checked() {
    let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow";
    let entropy = seed::parse_words(phrase).unwrap();
    // Any case and spacing; four letters of each word are enough.
    assert_eq!(
        seed::parse_words(
            "  LEGAL winner\tthank year wave  sausage worth useful legal winner thank YELLOW\n"
        )
        .unwrap(),
        entropy
    );
    assert_eq!(
        seed::parse_words("lega winn than year wave saus wort usef lega winn than yell").unwrap(),
        entropy
    );
    // A wrong word, a wrong count, a wrong checksum, words swapped.
    assert!(
        seed::parse_words(
            "legal winner thank year wave sausage worth useful legal winner thank yelow"
        )
        .is_err()
    );
    assert!(
        seed::parse_words("legal winner thank year wave sausage worth useful legal winner thank")
            .is_err()
    );
    assert!(
        seed::parse_words(
            "legal winner thank year wave sausage worth useful legal winner thank legal"
        )
        .is_err()
    );
    assert!(
        seed::parse_words(
            "winner legal thank year wave sausage worth useful legal winner thank yellow"
        )
        .is_err()
    );
    // Three letters aren't enough to know a word.
    assert!(
        seed::parse_words(
            "leg winner thank year wave sausage worth useful legal winner thank yellow"
        )
        .is_err()
    );
    assert!(seed::looks_like_words(phrase));
    assert!(!seed::looks_like_words(
        "KyZpNDKnfs94vbrwhJneDi77V6jF64PWPF8x5cdJb8ifgg2DUc9d"
    ));
}

#[test]
fn new_phrases_are_twelve_words_that_read_back() {
    let entropy = seed::new_entropy();
    let phrase = seed::words(&entropy[..]).unwrap();
    assert_eq!(phrase.split(' ').count(), 12);
    assert_eq!(&seed::parse_words(&phrase).unwrap()[..], &entropy[..]);
    let key = seed::key_from_entropy(&entropy[..]).unwrap();
    assert_eq!(
        Key::from_bytes(key.bytes()).unwrap().destination(),
        key.destination()
    );
}
