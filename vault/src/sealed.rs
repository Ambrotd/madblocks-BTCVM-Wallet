//! The vault file: the wallet's secret encrypted with AES-256-GCM under a
//! key derived (HKDF-SHA256) from the gate's signature of a random
//! challenge. Every other field is bound to the ciphertext as associated
//! data, so none can be swapped without the file failing to open.
//!
//! Version 2 says what the secret is: a private key, or a recovery phrase's
//! entropy. Version 1 files held only a key and still open.

use crate::VaultError;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use zeroize::Zeroizing;

pub(crate) const FORMAT: &str = "madblocks-btcvm-wallet-vault";
pub(crate) const VERSION: u32 = 2;
/// Version 1: a raw key, and no `secret` field in the associated data.
const V1: u32 = 1;

/// A private key, 32 bytes.
pub(crate) const KEY: &str = "key";
/// A BIP 39 phrase's entropy, 16 to 32 bytes; its key is derived.
pub(crate) const BIP39: &str = "bip39";
const KDF_INFO: &[u8] = b"madblocks-btcvm-wallet vault v1 aes-256-gcm";

/// The vault file's contents. Hex for bytes, so the file can be read (but
/// not opened) by eye.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sealed {
    pub format: String,
    pub version: u32,
    /// Which gate unlocks it, such as "windows-hello".
    pub gate: String,
    /// The name of the gate's key.
    pub credential: String,
    /// The gate key's public key, hex: its signature is checked against it.
    pub public_key: String,
    /// What the gate signs, hex.
    pub challenge: String,
    pub salt: String,
    pub nonce: String,
    /// The wallet key and the GCM tag, hex.
    pub ciphertext: String,
    /// The wallet's address, to show before unlocking and to check after.
    pub address: String,
    /// What the ciphertext holds: "key" or "bip39". Absent in version 1.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub secret: String,
}

/// A fresh challenge and salt for a new vault.
pub(crate) fn fresh_challenge() -> ([u8; 32], [u8; 32]) {
    let mut challenge = [0u8; 32];
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut challenge);
    OsRng.fill_bytes(&mut salt);
    (challenge, salt)
}

/// The AES key a signature opens.
pub(crate) fn derive(signature: &[u8], salt: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(salt), signature)
        .expand(KDF_INFO, &mut key[..])
        .expect("32 bytes is a valid HKDF length");
    key
}

impl Sealed {
    /// Seals `secret`, a `kind` of secret, under `aes_key`, with the other
    /// fields as associated data.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn seal(
        gate: &str,
        credential: &str,
        public_key: &[u8],
        challenge: &[u8],
        salt: &[u8],
        aes_key: &[u8; 32],
        kind: &str,
        secret: &[u8],
        address: &str,
    ) -> Sealed {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let mut sealed = Sealed {
            format: FORMAT.into(),
            version: VERSION,
            gate: gate.into(),
            credential: credential.into(),
            public_key: hex::encode(public_key),
            challenge: hex::encode(challenge),
            salt: hex::encode(salt),
            nonce: hex::encode(nonce),
            ciphertext: String::new(),
            address: address.into(),
            secret: kind.into(),
        };
        let aad = sealed.associated_data();
        let ciphertext = Aes256Gcm::new(aes_key.into())
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: secret,
                    aad: &aad,
                },
            )
            .expect("encrypting a few bytes can't fail");
        sealed.ciphertext = hex::encode(ciphertext);
        sealed
    }

    /// What the ciphertext holds.
    pub(crate) fn kind(&self) -> &str {
        if self.version == V1 {
            KEY
        } else {
            &self.secret
        }
    }

    /// Opens the vault with `aes_key`. Any change to the file, or the wrong
    /// key, makes it fail.
    pub(crate) fn open(&self, aes_key: &[u8; 32]) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let nonce: [u8; 12] = self
            .bytes("nonce", &self.nonce)?
            .try_into()
            .map_err(|_| VaultError::Corrupt("the nonce is not 12 bytes".into()))?;
        let ciphertext = self.bytes("ciphertext", &self.ciphertext)?;
        let aad = self.associated_data();
        let plain = Zeroizing::new(
            Aes256Gcm::new(aes_key.into())
                .decrypt(
                    &Nonce::from(nonce),
                    Payload {
                        msg: &ciphertext,
                        aad: &aad,
                    },
                )
                .map_err(|_| {
                    VaultError::Corrupt("the vault doesn't open with this Windows Hello key".into())
                })?,
        );
        let fits = match self.kind() {
            KEY => plain.len() == 32,
            _ => (16..=32).contains(&plain.len()) && plain.len() % 4 == 0,
        };
        if !fits {
            return Err(VaultError::Corrupt(
                "the sealed secret has the wrong length".into(),
            ));
        }
        Ok(plain)
    }

    /// Checks the file is a vault this version understands.
    pub(crate) fn check_format(&self) -> Result<(), VaultError> {
        let known = match self.version {
            V1 => self.secret.is_empty(),
            VERSION => self.secret == KEY || self.secret == BIP39,
            _ => false,
        };
        if self.format != FORMAT || !known {
            return Err(VaultError::Corrupt(format!(
                "not a vault this version understands ({} v{})",
                self.format, self.version
            )));
        }
        Ok(())
    }

    pub(crate) fn bytes(&self, name: &str, value: &str) -> Result<Vec<u8>, VaultError> {
        hex::decode(value).map_err(|_| VaultError::Corrupt(format!("the {name} is not hex")))
    }

    /// Every field but the ciphertext, each with its length, so none can be
    /// changed or shifted into another.
    fn associated_data(&self) -> Vec<u8> {
        let mut aad = Vec::new();
        let version = self.version.to_string();
        let mut fields = vec![
            &self.format,
            &version,
            &self.gate,
            &self.credential,
            &self.public_key,
            &self.challenge,
            &self.salt,
            &self.nonce,
            &self.address,
        ];
        if self.version != V1 {
            fields.push(&self.secret);
        }
        for field in fields {
            aad.extend_from_slice(&(field.len() as u32).to_le_bytes());
            aad.extend_from_slice(field.as_bytes());
        }
        aad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version 1 file, encrypted as version 1 did: its associated data
    /// had no `secret` field. Wallets made before recovery phrases must
    /// still open.
    #[test]
    fn version_1_files_still_open() {
        let aes_key = [7u8; 32];
        let secret = [9u8; 32];
        let nonce = [5u8; 12];
        let mut v1 = Sealed {
            format: FORMAT.into(),
            version: 1,
            gate: "windows-hello".into(),
            credential: "madblocks-btcvm-wallet-0011223344556677".into(),
            public_key: "aa".into(),
            challenge: "bb".into(),
            salt: "cc".into(),
            nonce: hex::encode(nonce),
            ciphertext: String::new(),
            address: "bc1qexample".into(),
            secret: String::new(),
        };
        let mut aad = Vec::new();
        for field in [
            FORMAT,
            "1",
            &v1.gate,
            &v1.credential,
            &v1.public_key,
            &v1.challenge,
            &v1.salt,
            &v1.nonce,
            &v1.address,
        ] {
            aad.extend_from_slice(&(field.len() as u32).to_le_bytes());
            aad.extend_from_slice(field.as_bytes());
        }
        let ciphertext = Aes256Gcm::new(&aes_key.into())
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: &secret,
                    aad: &aad,
                },
            )
            .unwrap();
        v1.ciphertext = hex::encode(ciphertext);
        // As the file has it: no "secret" field at all.
        let json = serde_json::to_string(&v1).unwrap();
        assert!(!json.contains("\"secret\""));
        let read: Sealed = serde_json::from_str(&json).unwrap();
        read.check_format().unwrap();
        assert_eq!(read.kind(), KEY);
        assert_eq!(&read.open(&aes_key).unwrap()[..], &secret[..]);
    }

    #[test]
    fn version_2_names_what_it_holds() {
        let aes_key = [3u8; 32];
        let sealed = Sealed::seal(
            "windows-hello",
            "c",
            b"pk",
            b"ch",
            b"salt",
            &aes_key,
            BIP39,
            &[1u8; 16],
            "bc1qx",
        );
        assert_eq!((sealed.version, sealed.kind()), (2, BIP39));
        sealed.check_format().unwrap();
        assert_eq!(&sealed.open(&aes_key).unwrap()[..], &[1u8; 16]);
        // A version 1 file can't claim to hold a phrase.
        let mut odd = sealed.clone();
        odd.version = 1;
        assert!(odd.check_format().is_err());
    }
}
