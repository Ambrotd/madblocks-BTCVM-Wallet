//! The wallet's own settings and records, in `settings.json` beside the
//! vaults. Nothing here is secret, and nothing here is trusted for money: an
//! address shown is checked against its key whenever the vault opens, and
//! the address book is checked again each time it is read.
//!
//! The file stays readable by 0.2.0, the last version without DOGE, in case
//! someone opens it again: DOGE's records are saved apart, under names it
//! doesn't read, since it refuses a whole file with a chain it doesn't know
//! and takes every withdrawal for BTCVM's. In memory they are with the rest.

use crate::journal;
use btcvm_wallet_core::Coin;
use btcvm_wallet_core::book::Contact;
use btcvm_wallet_core::bridge::Signers;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
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
    /// The same for DogecoinVM's bridge.
    pub notified_doge_signer_change: Option<String>,
    /// The wallet shown, by id.
    pub active: Option<String>,
    /// The coin shown: "btc" or "doge". BTC when unset.
    pub coin: Option<String>,
    /// The currency values are shown in too: "EUR", "USD" or "none". Unset,
    /// it follows the language.
    pub fiat: Option<String>,
    pub wallets: Vec<WalletRecord>,
    #[serde(deserialize_with = "lenient")]
    pub address_book: Vec<Contact>,
    /// DOGE's contacts as saved; in memory, in `address_book`.
    #[serde(deserialize_with = "lenient", skip_serializing_if = "Vec::is_empty")]
    pub doge_address_book: Vec<Contact>,
    /// Rotations of the peg's signers the wallet checked, oldest first, from
    /// the set built into it. Each is checked again at every start.
    pub rotations: Vec<RotationProof>,
    /// The new peg of the last rotation the user has seen announced.
    pub rotation_seen: Option<String>,
    /// Check the active wallet's Bitcoin balance with mempool.space too,
    /// which then learns the address. Off unless the user turns it on.
    pub check_balances: bool,
    /// Don't look for updates. They are looked for unless the user says.
    pub updates_off: bool,

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
    /// Whether Windows certified its Windows Hello key to be in a TPM, once
    /// asked: None until then, or while Windows can't tell.
    pub hardware: Option<bool>,
    /// The user has seen that Windows can't certify it.
    pub software_key_seen: bool,
    /// Its DOGE address, for a wallet with a recovery phrase, once learned
    /// from the unlocked phrase. A key's follows from its BTC address. Shown
    /// only: signing checks it against the key.
    pub doge_address: Option<String>,
    pub outgoing: Vec<Outgoing>,
    pub withdrawals: Vec<Withdrawal>,
    /// DOGE's payments and withdrawals as saved; in memory, in `outgoing`
    /// and `withdrawals`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub doge_outgoing: Vec<Outgoing>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub doge_withdrawals: Vec<Withdrawal>,
}

/// A list read entry by entry: an entry this version doesn't understand (a
/// later version's chain, say) is left out instead of failing the whole
/// file.
fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Vec<T>, D::Error> {
    let raw: Vec<serde_json::Value> = Vec::deserialize(d)?;
    Ok(raw
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

/// Whether a payment record is on DOGE's chains.
fn is_doge_chain(chain: &str) -> bool {
    matches!(chain, "dogecoin" | "dogecoinvm")
}

impl Settings {
    /// DOGE's records, saved apart, back with the rest.
    fn merge(&mut self) {
        let doge = std::mem::take(&mut self.doge_address_book);
        self.address_book.extend(doge);
        for w in &mut self.wallets {
            let outgoing = std::mem::take(&mut w.doge_outgoing);
            w.outgoing.extend(outgoing);
            let withdrawals = std::mem::take(&mut w.doge_withdrawals);
            w.withdrawals.extend(withdrawals.into_iter().map(|mut x| {
                x.coin = "doge".into();
                x
            }));
        }
    }

    /// As saved: DOGE's records apart from those 0.2.0 reads.
    fn split(&self) -> Settings {
        let mut s = self.clone();
        let (doge, btc): (Vec<Contact>, Vec<Contact>) = s
            .address_book
            .into_iter()
            .partition(|c| c.chain.coin() == Coin::Doge);
        s.address_book = btc;
        s.doge_address_book = doge;
        for w in &mut s.wallets {
            let (doge, btc): (Vec<Outgoing>, Vec<Outgoing>) = std::mem::take(&mut w.outgoing)
                .into_iter()
                .partition(|o| is_doge_chain(&o.chain));
            w.outgoing = btc;
            w.doge_outgoing = doge;
            let (doge, btc): (Vec<Withdrawal>, Vec<Withdrawal>) =
                std::mem::take(&mut w.withdrawals)
                    .into_iter()
                    .partition(|x| !x.coin.is_empty());
            w.withdrawals = btc;
            w.doge_withdrawals = doge;
        }
        s
    }
}

/// A rotation of the peg's signers: a move of the old set's coins to the new
/// set, signed by the old set (see the core's `rotation`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotationProof {
    /// "btcvm" or "bitcoin": where the move is.
    pub chain: String,
    pub from: Signers,
    pub to: Signers,
    /// The move, hex.
    pub tx: String,
    /// The transaction that made the coin whose signatures were checked,
    /// hex, by txid.
    pub prev: BTreeMap<String, String>,
    /// Unix seconds.
    pub verified: u64,
}

/// A payment a wallet sent, kept until it confirms: the coins it spends, so
/// they aren't offered again, and its change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outgoing {
    pub txid: String,
    /// "bitcoin", "btcvm", "dogecoin" or "dogecoinvm".
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

/// A withdrawal to the coin's own chain, followed until the bridge's payout
/// confirms.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Withdrawal {
    /// "doge" for DogecoinVM's bridge; empty for BTCVM's, as before DOGE.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coin: String,
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

/// Reads the settings; a missing file gives the defaults. An unreadable one
/// gives them too, but is kept aside first, never written over: the wallets'
/// names, the address book and the payments in flight are in it. A
/// single-wallet install's vault becomes the first wallet.
pub fn load(dir: &Path) -> Settings {
    let file = path(dir);
    let mut s: Settings = match fs::read(&file) {
        Err(_) => Settings::default(),
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let aside = dir.join(format!("settings.unreadable-{secs}.json"));
            let kept = fs::rename(&file, &aside).is_ok();
            journal::warn(&format!(
                "the settings don't read ({e}); {}",
                if kept {
                    format!("kept as {}", aside.display())
                } else {
                    "and can't be kept aside".into()
                }
            ));
            Settings::default()
        }),
    };
    s.merge();
    if s.wallets.is_empty() && dir.join("vault.json").exists() {
        s.wallets.push(WalletRecord {
            id: FIRST.into(),
            backup_confirmed: s.backup_confirmed,
            backup_required: s.backup_required,
            outgoing: std::mem::take(&mut s.outgoing),
            withdrawals: std::mem::take(&mut s.withdrawals),
            ..WalletRecord::default()
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
    f.write_all(&serde_json::to_vec_pretty(&settings.split()).expect("serializes"))?;
    f.sync_all()?;
    drop(f);
    fs::rename(tmp, path(dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use btcvm_wallet_core::Chain;

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

    const BTC_ADDRESS: &str = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
    const DOGE_ADDRESS: &str = "DBus3bamQjgJULBJtYXpEzDWQRwF5iwxgC";

    fn outgoing(txid: &str, chain: &str) -> Outgoing {
        Outgoing {
            txid: txid.into(),
            chain: chain.into(),
            kind: "send".into(),
            to: "x".into(),
            amount: 1,
            change: 0,
            spent: Vec::new(),
            time: 5,
        }
    }

    fn withdrawal(txid: &str, coin: &str) -> Withdrawal {
        Withdrawal {
            coin: coin.into(),
            txid: txid.into(),
            to: "x".into(),
            amount: 1,
            time: 5,
            status: "pending".into(),
            pays: None,
            payment_txid: None,
            payment_confirmations: None,
        }
    }

    #[test]
    fn doge_records_are_saved_where_0_2_0_doesnt_read_them() {
        let dir = temp_dir();
        let contact = |name: &str, address: &str, chain| Contact {
            name: name.into(),
            address: address.into(),
            chain,
        };
        let s = Settings {
            address_book: vec![
                contact("Exchange", BTC_ADDRESS, Chain::Bitcoin),
                contact("Shibe", DOGE_ADDRESS, Chain::Dogecoin),
            ],
            wallets: vec![WalletRecord {
                id: FIRST.into(),
                outgoing: vec![outgoing("aa", "bitcoin"), outgoing("bb", "dogecoinvm")],
                withdrawals: vec![withdrawal("cc", ""), withdrawal("dd", "doge")],
                ..WalletRecord::default()
            }],
            ..Settings::default()
        };
        save(&dir, &s).unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path(&dir)).unwrap()).unwrap();
        // What 0.2.0 reads holds only what it understands.
        let chains = |v: &serde_json::Value| -> Vec<String> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|e| e["chain"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(chains(&written["addressBook"]), ["bitcoin"]);
        assert_eq!(chains(&written["dogeAddressBook"]), ["dogecoin"]);
        let w = &written["wallets"][0];
        assert_eq!(chains(&w["outgoing"]), ["bitcoin"]);
        assert_eq!(chains(&w["dogeOutgoing"]), ["dogecoinvm"]);
        assert_eq!(w["withdrawals"].as_array().unwrap().len(), 1);
        assert!(w["withdrawals"][0].get("coin").is_none());
        assert_eq!(w["dogeWithdrawals"][0]["coin"], "doge");

        // Read again, they are all together, as before.
        let again = load(&dir);
        assert_eq!(again.address_book.len(), 2);
        assert!(again.doge_address_book.is_empty());
        let w = &again.wallets[0];
        assert_eq!(w.outgoing.len(), 2);
        assert_eq!(
            w.withdrawals
                .iter()
                .map(|x| x.coin.as_str())
                .collect::<Vec<_>>(),
            ["", "doge"]
        );
        assert!(w.doge_outgoing.is_empty() && w.doge_withdrawals.is_empty());
    }

    #[test]
    fn an_entry_of_a_later_version_is_left_out_not_the_file() {
        let dir = temp_dir();
        fs::write(
            path(&dir),
            format!(
                r#"{{"language":"es","wallets":[{{"id":"main","name":"Ahorro"}}],
                    "addressBook":[{{"name":"Exchange","address":"{BTC_ADDRESS}","chain":"bitcoin"}},
                                   {{"name":"Lite","address":"ltc1qxyz","chain":"litecoin"}}]}}"#
            ),
        )
        .unwrap();
        let s = load(&dir);
        assert_eq!(s.language.as_deref(), Some("es"));
        assert_eq!(s.wallets[0].name, "Ahorro");
        assert_eq!(s.address_book.len(), 1);
        assert_eq!(s.address_book[0].name, "Exchange");
    }

    #[test]
    fn an_unreadable_file_is_kept_aside_not_written_over() {
        let dir = temp_dir();
        fs::write(path(&dir), "{ not json").unwrap();
        let s = load(&dir);
        assert!(s.wallets.is_empty());
        assert!(!path(&dir).exists(), "moved aside");
        let kept: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("settings.unreadable-"))
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(
            fs::read_to_string(dir.join(&kept[0])).unwrap(),
            "{ not json"
        );
    }
}
