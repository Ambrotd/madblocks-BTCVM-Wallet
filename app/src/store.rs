//! The wallet's own settings and records, in `settings.json` beside the
//! vault. Nothing here is secret, and nothing here is trusted for money: the
//! address shown is checked against the key whenever the vault opens.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// The bridge server, when not the default.
    pub server: Option<String>,
    /// "es" or "en"; the system's language when unset.
    pub language: Option<String>,
    /// The key has been shown and the user said they saved it.
    pub backup_confirmed: bool,
    /// A wallet made here: nothing can be received until it's backed up.
    pub backup_required: bool,
    pub outgoing: Vec<Outgoing>,
    pub withdrawals: Vec<Withdrawal>,
    /// The new peg of a signer change already notified, so it's said once.
    pub notified_signer_change: Option<String>,
}

/// A payment this wallet sent, kept until it confirms: the coins it spends,
/// so they aren't offered again, and its change.
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

/// Reads the settings; a missing or unreadable file gives the defaults.
pub fn load(dir: &Path) -> Settings {
    fs::read(path(dir))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
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
