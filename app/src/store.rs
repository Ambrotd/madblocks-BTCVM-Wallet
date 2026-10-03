//! The wallet's own settings and records, in `settings.json` beside the
//! vaults. Nothing here is secret, and nothing here is trusted for money: an
//! address shown is checked against its key whenever the vault opens, and
//! the address book is checked again each time it is read.

use btcvm_wallet_core::book::Contact;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The first wallet's id. Its vault stays where single-wallet versions put
/// it, beside this file; later wallets live in `wallets/<id>/`.
pub const FIRST: &str = "main";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// The bridge server, when not the default.
    pub server: Option<String>,
    /// "es" or "en"; the system's language when unset.
    pub language: Option<String>,
    /// The new peg of a signer change already notified, so it's said once.
    pub notified_signer_change: Option<String>,
    /// The wallet shown, by id.
    pub active: Option<String>,
    pub wallets: Vec<WalletRecord>,
    pub address_book: Vec<Contact>,

    // Single-wallet versions kept these for their only wallet. They are read
    // once, moved into its record, and never written again.
    #[serde(skip_serializing)]
    pub backup_confirmed: bool,
    #[serde(skip_serializing)]
    pub backup_required: bool,
    #[serde(skip_serializing)]
    pub outgoing: Vec<Outgoing>,
    #[serde(skip_serializing)]
    pub withdrawals: Vec<Withdrawal>,
}

/// One wallet: its name and what the app keeps for it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WalletRecord {
    pub id: String,
    /// Empty for the window's default name.
    pub name: String,
    /// Unix seconds.
    pub created: u64,
    /// The key has been shown and the user said they saved it.
    pub backup_confirmed: bool,
    /// Made here: nothing can be received until it's backed up.
    pub backup_required: bool,
    pub outgoing: Vec<Outgoing>,
    pub withdrawals: Vec<Withdrawal>,
}

/// A payment a wallet sent, kept until it confirms: the coins it spends, so
/// they aren't offered again, and its change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outgoing {
    pub txid: String,
    /// "bitcoin" or "btcvm".
    pub chain: String,
    /// "send", "deposit" or "withdraw".
    pub kind: String,
    pub to: String,
    pub amount: u64,
    pub change: u64,
    /// txid:vout of each coin spent.
    pub spent: Vec<String>,
    /// Unix seconds.
    pub time: u64,
}

/// A withdrawal to Bitcoin, followed until the bridge's payout confirms.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Withdrawal {
    pub txid: String,
    pub to: String,
    pub amount: u64,
    pub time: u64,
    /// sending, pending, paid or unknown.
    pub status: String,
    pub pays: Option<String>,
    pub payment_txid: Option<String>,
    pub payment_confirmations: Option<i64>,
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

/// Where a wallet's vault lives.
pub fn vault_dir(dir: &Path, id: &str) -> PathBuf {
    if id == FIRST {
        dir.to_path_buf()
    } else {
        dir.join("wallets").join(id)
    }
}

/// Reads the settings; a missing or unreadable file gives the defaults. A
/// single-wallet install's vault becomes the first wallet.
pub fn load(dir: &Path) -> Settings {
    let mut s: Settings = fs::read(path(dir))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if s.wallets.is_empty() && dir.join("vault.json").exists() {
        s.wallets.push(WalletRecord {
            id: FIRST.into(),
            name: String::new(),
            created: 0,
            backup_confirmed: s.backup_confirmed,
            backup_required: s.backup_required,
            outgoing: std::mem::take(&mut s.outgoing),
            withdrawals: std::mem::take(&mut s.withdrawals),
        });
    }
    if !s
        .active
        .as_ref()
        .is_some_and(|a| s.wallets.iter().any(|w| &w.id == a))
    {
        s.active = s.wallets.first().map(|w| w.id.clone());
    }
    s
}

/// Writes the settings through a temporary file, so a crash never leaves
/// half of one.
pub fn save(dir: &Path, settings: &Settings) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join("settings.json.tmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(settings).expect("serializes"))?;
    f.sync_all()?;
    drop(f);
    fs::rename(tmp, path(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory: the process and the time make it unique.
    fn temp_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("btcvm-store-test-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_single_wallet_install_becomes_the_first_wallet() {
        let dir = temp_dir();
        fs::write(dir.join("vault.json"), "{}").unwrap();
        fs::write(
            path(&dir),
            r#"{"language":"es","backupConfirmed":true,"backupRequired":false,
                "outgoing":[{"txid":"aa","chain":"bitcoin","kind":"send","to":"x","amount":1,"change":0,"spent":[],"time":5}],
                "withdrawals":[]}"#,
        )
        .unwrap();
        let s = load(&dir);
        assert_eq!(s.wallets.len(), 1);
        let w = &s.wallets[0];
        assert_eq!(
            (w.id.as_str(), w.backup_confirmed, w.outgoing.len()),
            (FIRST, true, 1)
        );
        assert_eq!(s.active.as_deref(), Some(FIRST));
        assert_eq!(s.language.as_deref(), Some("es"));
        assert_eq!(vault_dir(&dir, FIRST), dir);

        // Written back, the old fields are gone and the wallet stays.
        save(&dir, &s).unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path(&dir)).unwrap()).unwrap();
        for old in [
            "backupConfirmed",
            "backupRequired",
            "outgoing",
            "withdrawals",
        ] {
            assert!(written.get(old).is_none(), "{old} is written again");
        }
        let again = load(&dir);
        assert_eq!(again.wallets.len(), 1);
        assert!(again.wallets[0].backup_confirmed);
        assert_eq!(again.wallets[0].outgoing.len(), 1);
    }

    #[test]
    fn no_vault_no_wallets() {
        let dir = temp_dir();
        let s = load(&dir);
        assert!(s.wallets.is_empty() && s.active.is_none());
        assert_eq!(vault_dir(&dir, "abc"), dir.join("wallets").join("abc"));
    }
}
