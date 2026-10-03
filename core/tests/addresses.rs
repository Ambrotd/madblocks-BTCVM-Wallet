//! Addresses: BIP350's segwit vectors (which supersede BIP173's), and the
//! wallet's rules for what it will pay.

use btcvm_wallet_core::encoding::{segwit_decode, segwit_encode};
use btcvm_wallet_core::*;

/// BIP350, "Test vectors for v0-v16 native segregated witness addresses":
/// valid addresses and the output scripts they stand for.
const VALID: [(&str, &str); 8] = [
    (
        "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4",
        "0014751e76e8199196d454941c45d1b3a323f1433bd6",
    ),
    (
        "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7",
        "00201863143c14c5166804bd19203356da136c985678cd4d27a1b8c6329604903262",
    ),
    (
        "bc1pw508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7kt5nd6y",
        "5128751e76e8199196d454941c45d1b3a323f1433bd6751e76e8199196d454941c45d1b3a323f1433bd6",
    ),
    ("BC1SW50QGDZ25J", "6002751e"),
    (
        "bc1zw508d6qejxtdg4y5r3zarvaryvaxxpcs",
        "5210751e76e8199196d454941c45d1b3a323",
    ),
    (
        "tb1qqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesrxh6hy",
        "0020000000c4a5cad46221b2a187905e5266362b99d5e91c6ce24d165dab93e86433",
    ),
    (
        "tb1pqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesf3hn0c",
        "5120000000c4a5cad46221b2a187905e5266362b99d5e91c6ce24d165dab93e86433",
    ),
    (
        "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0",
        "512079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    ),
];

/// BIP350's invalid segwit addresses, each with its reason.
const INVALID: [&str; 16] = [
    "tc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq5zuyut", // invalid human-readable part
    "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqh2y7hd", // bech32 instead of bech32m
    "tb1z0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqglt7rf", // bech32 instead of bech32m
    "BC1S0XLXVLHEMJA6C4DQV22UAPCTQUPFHLXM9H8Z3K2E72Q4K9HCZ7VQ54WELL", // bech32 instead of bech32m
    "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kemeawh",                     // bech32m instead of bech32
    "tb1q0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq24jc47", // bech32m instead of bech32
    "bc1p38j9r5y49hruaue7wxjce0updqjuyyx0kh56v8s25huc6995vvpql3jow4", // invalid character in checksum
    "BC130XLXVLHEMJA6C4DQV22UAPCTQUPFHLXM9H8Z3K2E72Q4K9HCZ7VQ7ZWS8R", // invalid witness version
    "bc1pw5dgrnzv",                                                   // program length 1
    "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v8n0nx0muaewav253zgeav", // program length 41
    "BC1QR508D6QEJXTDG4Y5R3ZARVARYV98GJ9P", // program length for witness version 0 (BIP141)
    "tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq47Zagq", // mixed case
    "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v07qwwzcrf", // zero padding of more than 4 bits
    "tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vpggkg4j", // non-zero padding in 8-to-5 conversion
    "bc1gmk9yu",                                                      // empty data section
    "bc1",                                                            // nothing after the separator
];

fn hrp_of(address: &str) -> String {
    let lower = address.to_ascii_lowercase();
    lower[..lower.rfind('1').unwrap()].to_string()
}

#[test]
fn bip350_valid_addresses() {
    for (address, script) in VALID {
        let hrp = hrp_of(address);
        let (version, program) =
            segwit_decode(address, &hrp).unwrap_or_else(|e| panic!("{address}: {e}"));
        let op = if version == 0 { 0 } else { 0x50 + version };
        assert_eq!(
            hex::encode([&[op, program.len() as u8][..], &program].concat()),
            script,
            "{address}"
        );
        assert_eq!(
            segwit_encode(&hrp, version, &program),
            address.to_ascii_lowercase()
        );
    }
}

#[test]
fn bip350_invalid_addresses() {
    for address in INVALID {
        for hrp in ["bc", "tb"] {
            assert!(segwit_decode(address, hrp).is_err(), "{address} for {hrp}");
        }
    }
}

#[test]
fn the_wallet_pays_only_what_can_be_spent() {
    // Witness versions above 1, and v1 programs that aren't 32 bytes, are
    // valid addresses but nothing can spend them yet: refused.
    for (address, _) in VALID {
        if hrp_of(address) != "bc" {
            assert!(decode_address(address, &MAINNET).is_err(), "{address}");
            continue;
        }
        let (version, program) = segwit_decode(address, "bc").unwrap();
        let payable = matches!((version, program.len()), (0, 20) | (0, 32) | (1, 32));
        assert_eq!(
            decode_address(address, &MAINNET).is_ok(),
            payable,
            "{address}"
        );
    }
}

#[test]
fn other_networks_are_refused() {
    let testnet = "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7";
    let e = decode_address(testnet, &MAINNET).unwrap_err();
    assert!(e.message().contains("different network"), "{e}");
    assert!(decode_address(testnet, &TESTNET).is_ok());
    // The same key hash as a Litecoin and a Dogecoin address: valid
    // checksums, other networks.
    for other in [
        "ltc1qw508d6qejxtdg4y5r3zarvary0c5xw7kgmn4n9",
        "DFpN6QqFfUm3gKNaxN6tNcab1FArL9cZLE",
    ] {
        let e = decode_address(other, &MAINNET).unwrap_err();
        assert!(e.message().contains("different network"), "{other}: {e}");
    }
}

#[test]
fn base58_and_segwit_round_trip() {
    let program = hex::decode("751e76e8199196d454941c45d1b3a323f1433bd6").unwrap();
    for (kind, address) in [
        (Kind::P2pkh, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH"),
        (Kind::P2sh, "3CNHUhP3uyB9EUtRLsmvFUmvGdjGdkTxJw"),
        (Kind::P2wpkh, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"),
    ] {
        let d = Destination::new(kind, &program).unwrap();
        assert_eq!(d.address(&MAINNET), address);
        assert_eq!(decode_address(address, &MAINNET).unwrap(), d);
        assert_eq!(Destination::from_script(&d.pk_script()), Some(d.clone()));
        assert_eq!(Destination::from_tag_bytes(&d.tag_bytes()), Some(d));
    }
    // A typo is caught by the checksum.
    assert!(decode_address("1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMJ", &MAINNET).is_err());
    assert!(decode_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t5", &MAINNET).is_err());
}
