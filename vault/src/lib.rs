//! The wallet key's vault.
//!
//! The key is secp256k1, which neither the TPM nor Windows Hello can hold
//! directly. So it lives in a file, encrypted with a key that only a Windows
//! Hello signature produces. A Windows Hello key, which the TPM holds and
//! never lets out, signs a random challenge. The signature is checked as
//! RSASSA-PKCS1-v1_5 under that key's public key, which also makes it
//! deterministic (the same every time), and HKDF turns it into the AES-256-GCM
//! key. Each signature asks for Windows Hello: PIN, fingerprint or face. The
//! file is useless on another PC, or to another user on this one.
//!
//! This is the design of the macOS wallets' Secure Enclave vault, with
//! Windows Hello in place of Touch ID. As there, a vault that can't be opened
//! any more is set aside rather than deleted, and replaced from the backup.

#![forbid(unsafe_code)]

#[cfg(all(windows, feature = "windows-hello"))]
mod hello;
mod sealed;

#[cfg(all(windows, feature = "windows-hello"))]
pub use hello::WindowsHello;
pub use sealed::Sealed;

use btcvm_wallet_core::{Key, MAINNET};
use rand_core::{OsRng, RngCore};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

/// Why the vault couldn't do what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    /// There is no vault on this PC.
    Missing,
    /// The vault file is damaged, or isn't this wallet's.
    Corrupt(String),
    /// Windows Hello's key for this vault is gone: Windows Hello was reset,
    /// or the file came from another PC. Only the backup can bring the
    /// wallet back.
    GateMissing,
    /// The user canceled Windows Hello.
    Canceled,
    /// Windows Hello isn't set up, or the security device is busy or
    /// locked. Nothing is lost; try again.
    Unavailable(String),
    /// A wallet is already stored here.
    Exists,
    Other(String),
}

impl VaultError {
    /// Whether only the backup can help: the vault can't be opened again.
    pub fn needs_restore(&self) -> bool {
        matches!(
            self,
            VaultError::Missing | VaultError::Corrupt(_) | VaultError::GateMissing
        )
    }
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::Missing => f.write_str("there is no wallet on this PC"),
            VaultError::Corrupt(why) => write!(f, "the wallet's vault is damaged: {why}"),
            VaultError::GateMissing => f.write_str(
                "the Windows Hello key that unlocks this wallet is gone; restore the wallet from its backup",
            ),
            VaultError::Canceled => f.write_str("Windows Hello was canceled"),
            VaultError::Unavailable(why) => f.write_str(why),
            VaultError::Exists => {
                f.write_str("this PC already has a wallet; remove it first")
            }
            VaultError::Other(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for VaultError {}

/// What unlocks a vault: a key that signs only with the user present, held
/// where it can't be copied, and signing a message the same way every time.
pub trait Gate {
    /// Recorded in the vault file, such as "windows-hello".
    fn kind(&self) -> &'static str;

    /// Makes a new key named `name` and returns its public key. May ask the
    /// user.
    fn create(&self, name: &str) -> Result<Vec<u8>, VaultError>;

    /// Signs `challenge` with the key `name`, asking the user. The result
    /// must be checked against `public_key`, so that it is that key's, and
    /// the same each time.
    fn sign(
        &self,
        name: &str,
        challenge: &[u8],
        public_key: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, VaultError>;

    /// Removes the key `name`.
    fn delete(&self, name: &str) -> Result<(), VaultError>;
}

/// The vault in `dir`, unlocked through `gate`.
pub struct Vault<G: Gate> {
    dir: PathBuf,
    gate: G,
}

impl<G: Gate> Vault<G> {
    pub fn new(dir: impl Into<PathBuf>, gate: G) -> Vault<G> {
        Vault {
            dir: dir.into(),
            gate,
        }
    }

    fn file(&self) -> PathBuf {
        self.dir.join("vault.json")
    }

    fn staged(&self) -> PathBuf {
        self.dir.join("vault.json.staged")
    }

    pub fn has_key(&self) -> bool {
        self.file().exists()
    }

    /// The stored wallet's address, read without unlocking: for showing only.
    /// The app checks it against the key each time it unlocks.
    pub fn address(&self) -> Result<String, VaultError> {
        Ok(self.read(&self.file())?.address)
    }

    /// Stores a new wallet's key, under a new Windows Hello key.
    pub fn store(&self, key: &Key) -> Result<(), VaultError> {
        if self.has_key() {
            return Err(VaultError::Exists);
        }
        let file = self.file();
        self.seal_into(key, &file)
    }

    /// Decrypts the key, asking for Windows Hello. The key must be the one
    /// the vault says it holds.
    pub fn unlock(&self) -> Result<Key, VaultError> {
        let sealed = self.read(&self.file())?;
        self.open(&sealed)
    }

    /// Replaces a vault that can't be opened any more with `key`, from the
    /// backup, under a new Windows Hello key. The old file is kept beside it,
    /// in case the key can still be recovered from it some other way.
    pub fn restore(&self, key: &Key) -> Result<(), VaultError> {
        // When the old file can still be read, the backup must be its key.
        if let Ok(old) = self.read(&self.file()) {
            let restored = key.destination().address(&MAINNET);
            if restored != old.address {
                return Err(VaultError::Other(format!(
                    "that key is for {restored}, not this wallet ({}); remove the wallet first to use a different key",
                    old.address
                )));
            }
        }
        let staged = self.staged();
        self.seal_into(key, &staged)?;
        if self.has_key() {
            self.move_aside("unusable")?;
        }
        fs::rename(&staged, self.file()).map_err(io)
    }

    /// Removes the wallet from this PC: unlocks it first, so only its owner
    /// can, then deletes the file and its Windows Hello key. Without a
    /// backup, its coins are gone.
    pub fn remove(&self) -> Result<(), VaultError> {
        let sealed = self.read(&self.file())?;
        self.open(&sealed)?;
        fs::remove_file(self.file()).map_err(io)?;
        self.gate.delete(&sealed.credential)
    }

    /// Takes a vault that can't be opened off the wallet without deleting
    /// it, and returns where it went.
    pub fn set_aside(&self) -> Result<PathBuf, VaultError> {
        if !self.has_key() {
            return Err(VaultError::Missing);
        }
        self.move_aside("removed")
    }

    // --- internals -------------------------------------------------------

    /// Seals `key` into `path` under a new gate key, then reads it back and
    /// opens it with the key just derived (no second prompt) before calling
    /// it done.
    fn seal_into(&self, key: &Key, path: &Path) -> Result<(), VaultError> {
        fs::create_dir_all(&self.dir).map_err(io)?;
        let mut id = [0u8; 8];
        OsRng.fill_bytes(&mut id);
        // Letters, digits and hyphens: Windows Hello refuses a name with "/".
        let credential = format!("madblocks-btcvm-wallet-{}", hex::encode(id));
        let public_key = self.gate.create(&credential)?;
        let result = (|| {
            let (challenge, salt) = sealed::fresh_challenge();
            let signature = self.gate.sign(&credential, &challenge, &public_key)?;
            let aes_key = sealed::derive(&signature, &salt);
            let address = key.destination().address(&MAINNET);
            let sealed = Sealed::seal(
                self.gate.kind(),
                &credential,
                &public_key,
                &challenge,
                &salt,
                &aes_key,
                key.bytes(),
                &address,
            );
            write_atomically(
                path,
                &serde_json::to_vec_pretty(&sealed).expect("serializes"),
            )?;
            let back = self.read(path)?;
            if back != sealed || *back.open(&aes_key)? != *key.bytes() {
                let _ = fs::remove_file(path);
                return Err(VaultError::Other(
                    "the vault didn't read back the same; nothing was stored".into(),
                ));
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = self.gate.delete(&credential);
        }
        result
    }

    fn open(&self, sealed: &Sealed) -> Result<Key, VaultError> {
        if sealed.gate != self.gate.kind() {
            return Err(VaultError::Corrupt(format!(
                "it is unlocked by {}, not {}",
                sealed.gate,
                self.gate.kind()
            )));
        }
        let public_key = sealed.bytes("public key", &sealed.public_key)?;
        let challenge = sealed.bytes("challenge", &sealed.challenge)?;
        let salt = sealed.bytes("salt", &sealed.salt)?;
        let signature = self
            .gate
            .sign(&sealed.credential, &challenge, &public_key)?;
        let secret = sealed.open(&sealed::derive(&signature, &salt))?;
        let key = Key::from_bytes(&secret[..])
            .map_err(|e| VaultError::Corrupt(e.message().to_string()))?;
        if key.destination().address(&MAINNET) != sealed.address {
            return Err(VaultError::Corrupt(
                "the key inside isn't the wallet's address".into(),
            ));
        }
        Ok(key)
    }

    fn read(&self, path: &Path) -> Result<Sealed, VaultError> {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(VaultError::Missing),
            Err(e) => return Err(io(e)),
        };
        let sealed: Sealed = serde_json::from_slice(&bytes)
            .map_err(|e| VaultError::Corrupt(format!("not a vault file: {e}")))?;
        sealed.check_format()?;
        Ok(sealed)
    }

    /// Renames the vault file to `vault.<label>-<n>.json`, never over an
    /// earlier one.
    fn move_aside(&self, label: &str) -> Result<PathBuf, VaultError> {
        for n in 1.. {
            let aside = self.dir.join(format!("vault.{label}-{n}.json"));
            if !aside.exists() {
                fs::rename(self.file(), &aside).map_err(io)?;
                return Ok(aside);
            }
        }
        unreachable!("an unused name exists")
    }
}

/// Where the wallet keeps its vault: `%LOCALAPPDATA%\madblocks BTCVM Wallet`,
/// which belongs to the user alone and doesn't roam or sync to OneDrive.
#[cfg(windows)]
pub fn default_dir() -> Result<PathBuf, VaultError> {
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| VaultError::Other("LOCALAPPDATA is not set".into()))?;
    Ok(PathBuf::from(base).join("madblocks BTCVM Wallet"))
}

/// Writes to a temporary file, flushes it to disk, then renames it into
/// place, so a crash leaves the old file or the new one, never half of one.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let tmp = path.with_extension("tmp");
    let mut f = fs::File::create(&tmp).map_err(io)?;
    f.write_all(bytes).map_err(io)?;
    f.sync_all().map_err(io)?;
    drop(f);
    fs::rename(&tmp, path).map_err(io)
}

fn io(e: std::io::Error) -> VaultError {
    VaultError::Other(format!("the vault's file: {e}"))
}
