//! The vault with a stand-in for Windows Hello, so it runs anywhere and asks
//! nobody. `examples/hello_check.rs` checks the real Windows Hello.

use btcvm_wallet_core::{Key, MAINNET};
use btcvm_wallet_vault::{Gate, Sealed, Vault, VaultError};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use zeroize::Zeroizing;

/// Windows Hello's stand-in: each key is a secret, its "public key" the
/// secret's hash, and its "signature" a hash of the secret and the message,
/// deterministic like RSA PKCS#1 v1.5. It counts the times it would ask.
#[derive(Default)]
struct FakeGate {
    keys: RefCell<HashMap<String, [u8; 32]>>,
    prompts: Cell<u32>,
    cancel: Cell<bool>,
}

impl Gate for &FakeGate {
    fn kind(&self) -> &'static str {
        "windows-hello"
    }

    fn create(&self, name: &str) -> Result<Vec<u8>, VaultError> {
        self.prompts.set(self.prompts.get() + 1);
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        self.keys.borrow_mut().insert(name.into(), secret);
        Ok(Sha256::digest(secret).to_vec())
    }

    fn sign(
        &self,
        name: &str,
        challenge: &[u8],
        public_key: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        if self.cancel.get() {
            return Err(VaultError::Canceled);
        }
        let secret = *self
            .keys
            .borrow()
            .get(name)
            .ok_or(VaultError::GateMissing)?;
        if Sha256::digest(secret)[..] != *public_key {
            return Err(VaultError::GateMissing);
        }
        self.prompts.set(self.prompts.get() + 1);
        Ok(Zeroizing::new(
            Sha256::new()
                .chain_update(secret)
                .chain_update(challenge)
                .finalize()
                .to_vec(),
        ))
    }

    fn delete(&self, name: &str) -> Result<(), VaultError> {
        self.keys.borrow_mut().remove(name);
        Ok(())
    }
}

/// A fresh directory under the system's temporary one.
fn temp_dir() -> PathBuf {
    let mut id = [0u8; 8];
    OsRng.fill_bytes(&mut id);
    let dir = std::env::temp_dir().join(format!("btcvm-vault-test-{}", hex::encode(id)));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn read(dir: &std::path::Path) -> Sealed {
    serde_json::from_slice(&std::fs::read(dir.join("vault.json")).unwrap()).unwrap()
}

fn write(dir: &std::path::Path, sealed: &Sealed) {
    std::fs::write(dir.join("vault.json"), serde_json::to_vec(sealed).unwrap()).unwrap();
}

#[test]
fn stores_and_unlocks_the_same_key() {
    let gate = FakeGate::default();
    let dir = temp_dir();
    let vault = Vault::new(&dir, &gate);
    assert!(!vault.has_key());
    assert_eq!(vault.unlock().unwrap_err(), VaultError::Missing);

    let key = Key::generate();
    vault.store(&key).unwrap();
    // Making the Windows Hello key, then its first signature.
    assert_eq!(gate.prompts.get(), 2);
    assert_eq!(
        vault.address().unwrap(),
        key.destination().address(&MAINNET)
    );

    let back = vault.unlock().unwrap();
    assert_eq!(back.bytes(), key.bytes());
    assert_eq!(gate.prompts.get(), 3, "one Windows Hello prompt per unlock");

    // The key is nowhere in the file.
    let file = std::fs::read_to_string(dir.join("vault.json")).unwrap();
    assert!(!file.contains(&hex::encode(key.bytes())));

    assert_eq!(
        vault.store(&Key::generate()).unwrap_err(),
        VaultError::Exists
    );
}

#[test]
fn any_change_to_the_file_stops_it_opening() {
    let gate = FakeGate::default();
    let dir = temp_dir();
    let vault = Vault::new(&dir, &gate);
    vault.store(&Key::generate()).unwrap();
    let good = read(&dir);
    let flip = |hex_value: &str| {
        let mut b = hex::decode(hex_value).unwrap();
        b[0] ^= 1;
        hex::encode(b)
    };
    let other = Key::generate().destination().address(&MAINNET);
    type Tamper<'a> = Box<dyn Fn(&mut Sealed) + 'a>;
    let tampered: Vec<(&str, Tamper)> = vec![
        ("challenge", Box::new(|s| s.challenge = flip(&s.challenge))),
        ("salt", Box::new(|s| s.salt = flip(&s.salt))),
        ("nonce", Box::new(|s| s.nonce = flip(&s.nonce))),
        (
            "ciphertext",
            Box::new(|s| s.ciphertext = flip(&s.ciphertext)),
        ),
        ("address", Box::new(move |s| s.address = other.clone())),
        (
            "public key",
            Box::new(|s| s.public_key = flip(&s.public_key)),
        ),
        ("credential", Box::new(|s| s.credential.push('x'))),
        ("gate", Box::new(|s| s.gate = "touch-id".into())),
        ("version", Box::new(|s| s.version = 2)),
    ];
    for (what, change) in tampered {
        let mut s = good.clone();
        change(&mut s);
        write(&dir, &s);
        let e = vault.unlock().unwrap_err();
        assert!(e.needs_restore(), "{what}: {e}");
    }
    write(&dir, &good);
    assert!(vault.unlock().is_ok());
}

#[test]
fn a_lost_windows_hello_key_is_restored_from_the_backup() {
    let gate = FakeGate::default();
    let dir = temp_dir();
    let vault = Vault::new(&dir, &gate);
    let key = Key::generate();
    vault.store(&key).unwrap();

    // Windows Hello was reset: its keys are gone.
    gate.keys.borrow_mut().clear();
    let e = vault.unlock().unwrap_err();
    assert_eq!(e, VaultError::GateMissing);
    assert!(e.needs_restore());

    // Only this wallet's own key restores it.
    assert!(vault.restore(&Key::generate()).is_err());
    vault
        .restore(&Key::parse(&key.wif(&MAINNET)).unwrap())
        .unwrap();
    assert_eq!(vault.unlock().unwrap().bytes(), key.bytes());
    // The unusable file was kept, not deleted.
    assert!(dir.join("vault.unusable-1.json").exists());
}

#[test]
fn canceling_windows_hello_loses_nothing() {
    let gate = FakeGate::default();
    let dir = temp_dir();
    let vault = Vault::new(&dir, &gate);
    let key = Key::generate();
    vault.store(&key).unwrap();
    gate.cancel.set(true);
    let e = vault.unlock().unwrap_err();
    assert_eq!(e, VaultError::Canceled);
    assert!(!e.needs_restore());
    gate.cancel.set(false);
    assert_eq!(vault.unlock().unwrap().bytes(), key.bytes());
}

#[test]
fn removing_takes_windows_hello_and_deletes_both() {
    let gate = FakeGate::default();
    let dir = temp_dir();
    let vault = Vault::new(&dir, &gate);
    vault.store(&Key::generate()).unwrap();
    gate.cancel.set(true);
    assert_eq!(vault.remove().unwrap_err(), VaultError::Canceled);
    assert!(vault.has_key(), "nothing is removed without Windows Hello");
    gate.cancel.set(false);
    vault.remove().unwrap();
    assert!(!vault.has_key());
    assert!(gate.keys.borrow().is_empty());
}

#[test]
fn an_unusable_vault_is_set_aside_not_deleted() {
    let gate = FakeGate::default();
    let dir = temp_dir();
    let vault = Vault::new(&dir, &gate);
    vault.store(&Key::generate()).unwrap();
    let aside = vault.set_aside().unwrap();
    assert!(aside.exists() && !vault.has_key());
    vault.store(&Key::generate()).unwrap();
    assert!(vault.set_aside().unwrap().ends_with("vault.removed-2.json"));
}
