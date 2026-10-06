//! The wallet key's vault.
//!
//! It keeps a private key or, for wallets made since recovery phrases, the
//! phrase's entropy, from which the key is derived (see the core's `seed`).
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

use btcvm_wallet_core::{
    Coin, DOGE_MAINNET, Destination, Key, Kind, MAINNET, decode_address, seed,
};
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

/// What a vault keeps: a private key, or a recovery phrase's entropy, whose
/// key is derived at m/84'/0'/0'/0/0.
pub enum Secret {
    Key(Key),
    Phrase(Zeroizing<Vec<u8>>),
}

impl Secret {
    /// A new twelve-word phrase.
    pub fn new_phrase() -> Secret {
        Secret::Phrase(Zeroizing::new(seed::new_entropy().to_vec()))
    }

    /// Reads a backup the user pasted: a recovery phrase, or a key (a
    /// compressed-key WIF or 64 hex digits).
    pub fn parse(text: &str) -> Result<Secret, btcvm_wallet_core::Error> {
        if seed::looks_like_words(text) {
            Ok(Secret::Phrase(seed::parse_words(text)?))
        } else {
            Ok(Secret::Key(Key::parse(text)?))
        }
    }

    /// The key it is, or makes, for BTC.
    pub fn key(&self) -> Result<Key, btcvm_wallet_core::Error> {
        self.key_for(Coin::Btc)
    }

    /// The key it is, or makes, for `coin`: a key serves every coin; a
    /// phrase makes BIP 84's for BTC and BIP 44's for DOGE.
    pub fn key_for(&self, coin: Coin) -> Result<Key, btcvm_wallet_core::Error> {
        match self {
            Secret::Key(k) => Key::from_bytes(k.bytes()),
            Secret::Phrase(entropy) => seed::key_for_coin(entropy, coin),
        }
    }

    /// The words, for a phrase.
    pub fn words(&self) -> Option<Zeroizing<String>> {
        match self {
            Secret::Key(_) => None,
            Secret::Phrase(entropy) => seed::words(entropy).ok(),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Secret::Key(_) => sealed::KEY,
            Secret::Phrase(_) => sealed::BIP39,
        }
    }

    fn bytes(&self) -> &[u8] {
        match self {
            Secret::Key(k) => &k.bytes()[..],
            Secret::Phrase(entropy) => entropy,
        }
    }
}

impl From<Key> for Secret {
    fn from(key: Key) -> Secret {
        Secret::Key(key)
    }
}

/// A gate key, as a vault names it: wallets can share one, each vault still
/// needing its own signature (of its own challenge) to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateKey {
    pub credential: String,
    pub public_key: Vec<u8>,
}

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

    /// Whether the key `name` is certified to be in hardware (a TPM): true
    /// or false when the gate can tell, None when it can't. Asks nothing.
    fn attested(&self, name: &str) -> Result<Option<bool>, VaultError> {
        let _ = name;
        Ok(None)
    }
}

/// A gate shared between vaults (an app's wallets) is still that gate.
impl<T: Gate + ?Sized> Gate for std::sync::Arc<T> {
    fn kind(&self) -> &'static str {
        (**self).kind()
    }

    fn create(&self, name: &str) -> Result<Vec<u8>, VaultError> {
        (**self).create(name)
    }

    fn sign(
        &self,
        name: &str,
        challenge: &[u8],
        public_key: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        (**self).sign(name, challenge, public_key)
    }

    fn delete(&self, name: &str) -> Result<(), VaultError> {
        (**self).delete(name)
    }

    fn attested(&self, name: &str) -> Result<Option<bool>, VaultError> {
        (**self).attested(name)
    }
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

    /// The wallet's address for `coin`, read without unlocking, for showing
    /// only. A key's DOGE address follows from its BTC one, as they share the
    /// key's hash. A phrase makes another key for DOGE, so its address is
    /// None here: the app learns it once the phrase is unlocked.
    pub fn address_for(&self, coin: Coin) -> Result<Option<String>, VaultError> {
        let file = self.read(&self.file())?;
        match coin {
            Coin::Btc => Ok(Some(file.address)),
            Coin::Doge if file.kind() == sealed::KEY => {
                let btc = decode_address(&file.address, &MAINNET).map_err(corrupt)?;
                let doge = Destination::new(Kind::P2pkh, btc.program()).map_err(corrupt)?;
                Ok(Some(doge.address(&DOGE_MAINNET)))
            }
            Coin::Doge => Ok(None),
        }
    }

    /// Stores a new wallet's secret under a new Windows Hello key: Windows
    /// Hello asks twice, to make the key and to sign with it.
    pub fn store(&self, secret: &Secret) -> Result<(), VaultError> {
        self.store_under(secret, None)
    }

    /// Stores a new wallet's secret under another vault's Windows Hello key:
    /// Windows Hello asks once.
    pub fn store_with(&self, secret: &Secret, gate_key: &GateKey) -> Result<(), VaultError> {
        self.store_under(secret, Some(gate_key))
    }

    fn store_under(&self, secret: &Secret, gate_key: Option<&GateKey>) -> Result<(), VaultError> {
        if self.has_key() {
            return Err(VaultError::Exists);
        }
        let file = self.file();
        self.seal_into(secret, &file, gate_key)
    }

    /// The Windows Hello key this vault is sealed under, read without
    /// unlocking.
    pub fn gate_key(&self) -> Result<GateKey, VaultError> {
        let sealed = self.read(&self.file())?;
        Ok(GateKey {
            public_key: sealed.bytes("public key", &sealed.public_key)?,
            credential: sealed.credential,
        })
    }

    /// Whether the vault's gate key is certified to be in hardware (a TPM).
    /// Asks nothing.
    pub fn attested(&self) -> Result<Option<bool>, VaultError> {
        let sealed = self.read(&self.file())?;
        self.gate.attested(&sealed.credential)
    }

    /// Decrypts the key, asking for Windows Hello. The key must be the one
    /// the vault says it holds.
    pub fn unlock(&self) -> Result<Key, VaultError> {
        self.unlock_secret()?.key().map_err(corrupt)
    }

    /// Decrypts what the vault keeps, for a backup, asking for Windows Hello.
    pub fn unlock_secret(&self) -> Result<Secret, VaultError> {
        let sealed = self.read(&self.file())?;
        self.open(&sealed)
    }

    /// Replaces a vault that can't be opened any more with `key`, from the
    /// backup, under a new Windows Hello key. The old file is kept beside it,
    /// in case the key can still be recovered from it some other way.
    pub fn restore(&self, secret: &Secret) -> Result<(), VaultError> {
        // When the old file can still be read, the backup must be its key.
        if let Ok(old) = self.read(&self.file()) {
            let restored = secret
                .key()
                .map_err(corrupt)?
                .destination()
                .address(&MAINNET);
            if restored != old.address {
                return Err(VaultError::Other(format!(
                    "that key is for {restored}, not this wallet ({}); remove the wallet first to use a different key",
                    old.address
                )));
            }
        }
        let staged = self.staged();
        self.seal_into(secret, &staged, None)?;
        if self.has_key() {
            self.move_aside("unusable")?;
        }
        fs::rename(&staged, self.file()).map_err(io)
    }

    /// Removes the wallet from this PC: unlocks it first, so only its owner
    /// can, then deletes the file, and its Windows Hello key unless another
    /// vault shares it (`keep_gate_key`). Without a backup, its coins are
    /// gone.
    pub fn remove(&self, keep_gate_key: bool) -> Result<(), VaultError> {
        let sealed = self.read(&self.file())?;
        self.open(&sealed)?;
        fs::remove_file(self.file()).map_err(io)?;
        if keep_gate_key {
            Ok(())
        } else {
            self.gate.delete(&sealed.credential)
        }
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

    /// Seals `secret` into `path` under `gate_key`, or a new one, then reads
    /// it back and opens it with the key just derived (no further prompt)
    /// before calling it done.
    fn seal_into(
        &self,
        secret: &Secret,
        path: &Path,
        gate_key: Option<&GateKey>,
    ) -> Result<(), VaultError> {
        fs::create_dir_all(&self.dir).map_err(io)?;
        let address = secret
            .key()
            .map_err(corrupt)?
            .destination()
            .address(&MAINNET);
        let (credential, public_key, made) = match gate_key {
            Some(g) => (g.credential.clone(), g.public_key.clone(), false),
            None => {
                let mut id = [0u8; 8];
                OsRng.fill_bytes(&mut id);
                // Letters, digits and hyphens: Windows Hello refuses "/".
                let credential = format!("madblocks-btcvm-wallet-{}", hex::encode(id));
                let public_key = self.gate.create(&credential)?;
                (credential, public_key, true)
            }
        };
        let result = (|| {
            let (challenge, salt) = sealed::fresh_challenge();
            let signature = self.gate.sign(&credential, &challenge, &public_key)?;
            let aes_key = sealed::derive(&signature, &salt);
            let sealed = Sealed::seal(
                self.gate.kind(),
                &credential,
                &public_key,
                &challenge,
                &salt,
                &aes_key,
                secret.kind(),
                secret.bytes(),
                &address,
            );
            write_atomically(
                path,
                &serde_json::to_vec_pretty(&sealed).expect("serializes"),
            )?;
            let back = self.read(path)?;
            if back != sealed || *back.open(&aes_key)? != *secret.bytes() {
                let _ = fs::remove_file(path);
                return Err(VaultError::Other(
                    "the vault didn't read back the same; nothing was stored".into(),
                ));
            }
            Ok(())
        })();
        if result.is_err() && made {
            let _ = self.gate.delete(&credential);
        }
        result
    }

    fn open(&self, sealed: &Sealed) -> Result<Secret, VaultError> {
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
        let plain = sealed.open(&sealed::derive(&signature, &salt))?;
        let secret = match sealed.kind() {
            sealed::KEY => Secret::Key(Key::from_bytes(&plain[..]).map_err(corrupt)?),
            _ => Secret::Phrase(plain),
        };
        if secret
            .key()
            .map_err(corrupt)?
            .destination()
            .address(&MAINNET)
            != sealed.address
        {
            return Err(VaultError::Corrupt(
                "the key inside isn't the wallet's address".into(),
            ));
        }
        Ok(secret)
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

fn corrupt(e: btcvm_wallet_core::Error) -> VaultError {
    VaultError::Corrupt(e.message().to_string())
}

fn io(e: std::io::Error) -> VaultError {
    VaultError::Other(format!("the vault's file: {e}"))
}
