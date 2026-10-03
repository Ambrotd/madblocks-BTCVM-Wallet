//! The vault file: the wallet key encrypted with AES-256-GCM under a key
//! derived (HKDF-SHA256) from the gate's signature of a random challenge.
//! Every other field is bound to the ciphertext as associated data, so none
//! can be swapped without the file failing to open.

use crate::VaultError;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use zeroize::Zeroizing;

pub(crate) const FORMAT: &str = "madblocks-btcvm-wallet-vault";
pub(crate) const VERSION: u32 = 1;
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
    /// Seals `secret` under `aes_key`, with the other fields as associated
    /// data.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn seal(
        gate: &str,
        credential: &str,
        public_key: &[u8],
        challenge: &[u8],
        salt: &[u8],
        aes_key: &[u8; 32],
        secret: &[u8; 32],
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
            .expect("encrypting 32 bytes can't fail");
        sealed.ciphertext = hex::encode(ciphertext);
        sealed
    }

    /// Opens the vault with `aes_key`. Any change to the file, or the wrong
    /// key, makes it fail.
    pub(crate) fn open(&self, aes_key: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
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
        if plain.len() != 32 {
            return Err(VaultError::Corrupt("the sealed key is not 32 bytes".into()));
        }
        let mut secret = Zeroizing::new([0u8; 32]);
        secret.copy_from_slice(&plain);
        Ok(secret)
    }

    /// Checks the file is a vault this version understands.
    pub(crate) fn check_format(&self) -> Result<(), VaultError> {
        if self.format != FORMAT || self.version != VERSION {
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
        for field in [
            &self.format,
            &version,
            &self.gate,
            &self.credential,
            &self.public_key,
            &self.challenge,
            &self.salt,
            &self.nonce,
            &self.address,
        ] {
            aad.extend_from_slice(&(field.len() as u32).to_le_bytes());
            aad.extend_from_slice(field.as_bytes());
        }
        aad
    }
}
