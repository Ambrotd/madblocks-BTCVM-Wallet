//! Recovery phrases: BIP 39 words, and the keys they make (BIP 32): at BIP
//! 84's first receiving address, m/84'/0'/0'/0/0, for BTC, and at BIP 44's
//! for Dogecoin, m/44'/3'/0'/0/0, for DOGE. Twelve words restore the same
//! Bitcoin address in Sparrow, Electrum (as a BIP 39 seed), BlueWallet and
//! any other wallet that follows those BIPs, and the same Dogecoin address
//! in Dogecoin wallets that take BIP 39 phrases; on the VMs, in this wallet.
//!
//! Only the English list, and no passphrase in the wallet: both keep every
//! phrase plain ASCII, which BIP 39's NFKD normalization leaves as it is.

use crate::encoding::sha256;
use crate::payment::Coin;
use crate::{Error, Key, Result, invalid};
use hmac::{Hmac, Mac};
use k256::elliptic_curve::PrimeField;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::{FieldBytes, Scalar, SecretKey};
use rand_core::{OsRng, RngCore};
use sha2::Sha512;
use std::sync::OnceLock;
use zeroize::Zeroizing;

/// BIP 39's English list, as published (SHA-256 2f5eed53…dbda).
const ENGLISH: &str = include_str!("bip39-english.txt");

const HARDENED: u32 = 1 << 31;

/// A BIP 32 node: a private key and its chain code.
pub type Node = (Zeroizing<[u8; 32]>, Zeroizing<[u8; 32]>);

/// BIP 84's first receiving address on Bitcoin: m/84'/0'/0'/0/0.
pub const BIP84_FIRST: [u32; 5] = [84 | HARDENED, HARDENED, HARDENED, 0, 0];
/// BIP 44's first receiving address on Dogecoin (coin type 3):
/// m/44'/3'/0'/0/0.
pub const BIP44_DOGE_FIRST: [u32; 5] = [44 | HARDENED, 3 | HARDENED, HARDENED, 0, 0];

pub(crate) fn wordlist() -> &'static [&'static str] {
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        ENGLISH
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .collect()
    })
}

/// Fresh entropy for a twelve-word phrase, from the operating system.
pub fn new_entropy() -> Zeroizing<[u8; 16]> {
    let mut entropy = Zeroizing::new([0u8; 16]);
    OsRng.fill_bytes(&mut entropy[..]);
    entropy
}

/// The phrase for `entropy` (16 to 32 bytes, in steps of 4), the words
/// separated by single spaces.
pub fn words(entropy: &[u8]) -> Result<Zeroizing<String>> {
    if !(16..=32).contains(&entropy.len()) || !entropy.len().is_multiple_of(4) {
        return invalid("a recovery phrase's entropy is 16 to 32 bytes, in steps of 4");
    }
    let hash = sha256(entropy);
    let entropy_bits = entropy.len() * 8;
    let bit = |i: usize| -> u32 {
        let byte = if i < entropy_bits {
            entropy[i / 8]
        } else {
            hash[(i - entropy_bits) / 8]
        };
        u32::from((byte >> (7 - i % 8)) & 1)
    };
    let count = (entropy_bits + entropy_bits / 32) / 11;
    let list = wordlist();
    let mut out = Zeroizing::new(String::with_capacity(count * 9));
    for w in 0..count {
        let index = (0..11).fold(0u32, |acc, b| (acc << 1) | bit(w * 11 + b));
        if w > 0 {
            out.push(' ');
        }
        out.push_str(list[index as usize]);
    }
    Ok(out)
}

/// Whether `text` looks like a phrase rather than a key: a dozen or more
/// words of letters.
pub fn looks_like_words(text: &str) -> bool {
    let mut words = text.split_whitespace();
    words.clone().count() >= 12 && words.all(|w| w.chars().all(|c| c.is_ascii_alphabetic()))
}

/// The entropy a phrase holds, checking every word and the checksum. Any
/// spacing and case are taken, and so is a word's first four letters, which
/// identify it in the English list.
pub fn parse_words(text: &str) -> Result<Zeroizing<Vec<u8>>> {
    let words: Zeroizing<Vec<String>> = Zeroizing::new(
        text.split_whitespace()
            .map(|w| w.to_ascii_lowercase())
            .collect(),
    );
    if ![12, 15, 18, 21, 24].contains(&words.len()) {
        return invalid("a recovery phrase has 12, 15, 18, 21 or 24 words");
    }
    let list = wordlist();
    let mut bits: Zeroizing<Vec<bool>> = Zeroizing::new(Vec::with_capacity(words.len() * 11));
    for (n, word) in words.iter().enumerate() {
        let index = match list.binary_search(&word.as_str()) {
            Ok(i) => i,
            Err(_) => {
                let mut found = list
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| word.len() >= 4 && w.starts_with(word.as_str()));
                match (found.next(), found.next()) {
                    (Some((i, _)), None) => i,
                    _ => {
                        return Err(Error::Invalid(format!(
                            "word {} isn't one of the BIP 39 words",
                            n + 1
                        )));
                    }
                }
            }
        };
        bits.extend((0..11).rev().map(|b| (index >> b) & 1 == 1));
    }
    let checksum_bits = bits.len() / 33;
    let entropy_bits = bits.len() - checksum_bits;
    let mut entropy = Zeroizing::new(vec![0u8; entropy_bits / 8]);
    for (i, set) in bits[..entropy_bits].iter().enumerate() {
        if *set {
            entropy[i / 8] |= 1 << (7 - i % 8);
        }
    }
    let hash = sha256(&entropy);
    for i in 0..checksum_bits {
        if bits[entropy_bits + i] != ((hash[i / 8] >> (7 - i % 8)) & 1 == 1) {
            return invalid(
                "the recovery phrase's checksum doesn't match: check each word and their order",
            );
        }
    }
    Ok(entropy)
}

/// BIP 39's seed: PBKDF2-HMAC-SHA512 of the phrase, 2048 rounds, salted with
/// "mnemonic" and the passphrase. One block of output is the whole seed.
pub fn seed(words: &str, passphrase: &str) -> Zeroizing<[u8; 64]> {
    let mac = Hmac::<Sha512>::new_from_slice(words.as_bytes()).expect("HMAC takes any key");
    let mut u = Zeroizing::new([0u8; 64]);
    let mut first = mac.clone();
    first.update(b"mnemonic");
    first.update(passphrase.as_bytes());
    first.update(&1u32.to_be_bytes());
    u.copy_from_slice(&first.finalize().into_bytes());
    let mut out = Zeroizing::new(*u);
    for _ in 1..2048 {
        let mut next = mac.clone();
        next.update(&u[..]);
        u.copy_from_slice(&next.finalize().into_bytes());
        for (o, x) in out.iter_mut().zip(u.iter()) {
            *o ^= x;
        }
    }
    out
}

/// The private key and chain code at `path` from `seed` (BIP 32).
pub fn extended_key(seed: &[u8], path: &[u32]) -> Result<Node> {
    let mut node = split(&hmac512(b"Bitcoin seed", &[seed]))?;
    for &index in path {
        node = child(&node.0, &node.1, index)?;
    }
    Ok(node)
}

/// The key a phrase's entropy makes at m/84'/0'/0'/0/0.
pub fn key_from_entropy(entropy: &[u8]) -> Result<Key> {
    key_for_coin(entropy, Coin::Btc)
}

/// The key a phrase's entropy makes for `coin`: at m/84'/0'/0'/0/0 for BTC,
/// at m/44'/3'/0'/0/0 for DOGE.
pub fn key_for_coin(entropy: &[u8], coin: Coin) -> Result<Key> {
    let path = match coin {
        Coin::Btc => &BIP84_FIRST,
        Coin::Doge => &BIP44_DOGE_FIRST,
    };
    let phrase = words(entropy)?;
    let (key, _) = extended_key(&seed(&phrase, "")[..], path)?;
    Key::from_bytes(&key[..])
}

/// BIP 32's private child derivation.
fn child(key: &[u8; 32], chain: &[u8; 32], index: u32) -> Result<Node> {
    let parent = scalar(key)?;
    let mut data = Zeroizing::new(Vec::with_capacity(37));
    if index >= HARDENED {
        data.push(0);
        data.extend_from_slice(key);
    } else {
        let secret = SecretKey::from_bytes(&FieldBytes::from(*key))
            .map_err(|_| Error::Invalid("not a valid private key".into()))?;
        data.extend_from_slice(secret.public_key().to_encoded_point(true).as_bytes());
    }
    data.extend_from_slice(&index.to_be_bytes());
    let i = hmac512(chain, &[&data]);
    let tweak = scalar(i[..32].try_into().expect("32 bytes"))?;
    let k = tweak + parent;
    if bool::from(k.is_zero()) {
        return invalid("this phrase derives an unusable key; use another");
    }
    let mut out = Zeroizing::new([0u8; 32]);
    out.copy_from_slice(&k.to_bytes());
    let mut chain = Zeroizing::new([0u8; 32]);
    chain.copy_from_slice(&i[32..]);
    Ok((out, chain))
}

/// A 32-byte number as a key: below the curve's order, and not zero.
fn scalar(bytes: &[u8; 32]) -> Result<Scalar> {
    let s: Option<Scalar> = Scalar::from_repr(FieldBytes::from(*bytes)).into();
    match s {
        Some(s) if !bool::from(s.is_zero()) => Ok(s),
        _ => invalid("this phrase derives an unusable key; use another"),
    }
}

fn split(i: &[u8; 64]) -> Result<Node> {
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&i[..32]);
    scalar(&key)?;
    let mut chain = Zeroizing::new([0u8; 32]);
    chain.copy_from_slice(&i[32..]);
    Ok((key, chain))
}

fn hmac512(key: &[u8], parts: &[&[u8]]) -> Zeroizing<[u8; 64]> {
    let mut mac = Hmac::<Sha512>::new_from_slice(key).expect("HMAC takes any key");
    for part in parts {
        mac.update(part);
    }
    let mut out = Zeroizing::new([0u8; 64]);
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_bip39s_english_list() {
        let list = wordlist();
        assert_eq!(list.len(), 2048);
        // As published, whatever line endings the checkout gave the file.
        let published = list.iter().map(|w| format!("{w}\n")).collect::<String>();
        assert_eq!(
            hex::encode(sha256(published.as_bytes())),
            "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda"
        );
        // Sorted, so binary search finds words; unique by four letters.
        assert!(list.windows(2).all(|w| w[0] < w[1]));
        let prefixes: std::collections::HashSet<&str> =
            list.iter().map(|w| &w[..w.len().min(4)]).collect();
        assert_eq!(prefixes.len(), 2048);
    }
}
