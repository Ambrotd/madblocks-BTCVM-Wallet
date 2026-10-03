//! The wallet behind the window: what it shows and what it can do.
//!
//! Every payment is planned by the core for review and kept here; the web
//! view gets a description and an id, never a key or anything it could
//! change before signing. Signing asks for Windows Hello, reads the signed
//! transaction back, and records the payment before it is sent.

use crate::api::{AddressView, ApiError, Bridge, BridgeStatus, DEFAULT_SERVER, DepositEntry};
use crate::store::{self, Outgoing, Settings, Withdrawal};
use btcvm_wallet_core::bridge::{self as peg, CheckSource, Pinned, peg_out_data};
use btcvm_wallet_core::tx::parse_tx;
use btcvm_wallet_core::{
    BridgeInfo, Chain, Coins, Destination, Key, Kind as AddressKind, MAINNET, Plan, Request,
    VerifiedBridge, about, decode_address, format_btc, max_payment, parse_btc, plan_deposit,
    plan_send, plan_withdrawal, sign_plan,
};
use btcvm_wallet_vault::{Vault, VaultError, WindowsHello};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

/// Why an action didn't happen, for the window to show.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub message: String,
    /// A server answer the wallet checked and found false: maybe an attack.
    pub untrusted: bool,
    /// The user canceled Windows Hello: nothing to report.
    pub canceled: bool,
}

pub type Outcome<T> = Result<T, Failure>;

fn fail<T>(message: impl Into<String>) -> Outcome<T> {
    Err(Failure {
        message: message.into(),
        untrusted: false,
        canceled: false,
    })
}

impl From<btcvm_wallet_core::Error> for Failure {
    fn from(e: btcvm_wallet_core::Error) -> Failure {
        Failure {
            untrusted: e.is_untrusted(),
            message: e.message().to_string(),
            canceled: false,
        }
    }
}

impl From<VaultError> for Failure {
    fn from(e: VaultError) -> Failure {
        Failure {
            canceled: e == VaultError::Canceled,
            message: e.to_string(),
            untrusted: false,
        }
    }
}

impl From<ApiError> for Failure {
    fn from(e: ApiError) -> Failure {
        Failure {
            message: e.message,
            untrusted: false,
            canceled: false,
        }
    }
}

#[derive(Default)]
struct Snapshot {
    info: Option<BridgeInfo>,
    bridge: Option<VerifiedBridge>,
    bridge_error: Option<Failure>,
    status: Option<BridgeStatus>,
    btcvm: Option<AddressView>,
    bitcoin: Option<AddressView>,
    bitcoin_note: Option<String>,
    deposits: Vec<DepositEntry>,
    connection_error: Option<String>,
    updated: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Send,
    Deposit,
    Withdraw,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Send => "send",
            Kind::Deposit => "deposit",
            Kind::Withdraw => "withdraw",
        }
    }
}

/// A payment waiting for the user's confirmation.
#[derive(Clone)]
struct Pending {
    id: u64,
    kind: Kind,
    plan: Plan,
    to: String,
    amount: u64,
}

pub struct Wallet {
    dir: PathBuf,
    vault: Vault<WindowsHello>,
    settings: Mutex<Settings>,
    bridge: Mutex<Arc<Bridge>>,
    snapshot: Mutex<Snapshot>,
    pending: Mutex<Option<Pending>>,
    refreshing: Mutex<()>,
    refresh_again: AtomicBool,
    next_id: AtomicU64,
    language: Mutex<String>,
}

// --- what the window shows ---------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub version: &'static str,
    pub server: String,
    pub language: String,
    pub has_wallet: bool,
    /// Set when the vault can't be opened and only the backup can help.
    pub vault_problem: Option<String>,
    /// Hidden while a new wallet's key isn't backed up.
    pub address: Option<String>,
    pub receive_blocked: bool,
    pub backup_confirmed: bool,
    pub bitcoin: Option<Balance>,
    pub btcvm: Option<Balance>,
    pub bitcoin_note: Option<String>,
    pub history: Vec<HistoryRow>,
    pub deposits: Vec<DepositEntry>,
    pub withdrawals: Vec<Withdrawal>,
    pub in_flight: Vec<Outgoing>,
    pub bridge: BridgeView,
    pub connection_error: Option<String>,
    pub updated: u64,
}

#[derive(Serialize, Clone)]
pub struct Balance {
    pub confirmed: String,
    pub pending: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRow {
    pub chain: &'static str,
    pub txid: String,
    pub net: String,
    pub confirmations: i64,
    pub time: Option<i64>,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BridgeView {
    pub connected: bool,
    pub trusted: bool,
    pub error: Option<Failure>,
    pub signer_change: Option<SignerChangeView>,
    pub peg_address: Option<String>,
    pub min_deposit: Option<String>,
    pub max_deposit: Option<String>,
    pub min_peg_out: Option<String>,
    pub fee_rate: Option<u64>,
    pub vm_fee: Option<String>,
    pub payout_fee: Option<String>,
    pub paused: bool,
    pub solvent: Option<bool>,
    pub locked: Option<String>,
    pub circulating: Option<String>,
    pub bitcoin_syncing: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignerChangeView {
    pub trusted_peg: String,
    pub reported_peg: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub links: Vec<LinkView>,
}

#[derive(Serialize, Clone)]
pub struct LinkView {
    pub source: &'static str,
    pub url: String,
}

/// A payment for review: everything it does, in plain terms.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub id: u64,
    pub kind: &'static str,
    pub chain: &'static str,
    /// Who is paid: the address typed, the deposit address, or for a
    /// withdrawal the Bitcoin address the bridge pays.
    pub to: String,
    pub amount: String,
    pub fee: String,
    /// What leaves the wallet: the amount and the fee.
    pub total: String,
    pub outputs: Vec<OutputRow>,
    /// A deposit: what BTCVM credits, and after how many confirmations.
    pub credited: Option<String>,
    pub confirmations: Option<u32>,
    /// A withdrawal: the Bitcoin fee taken from the payout at today's rate.
    pub payout_fee: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputRow {
    pub value: String,
    pub address: Option<String>,
    /// pay, deposit, reserve, change, tag or other.
    pub role: &'static str,
    pub withdrawal_to: Option<String>,
}

#[derive(Serialize)]
pub struct Sent {
    pub txid: String,
    pub chain: &'static str,
}

fn chain_name(chain: Chain) -> &'static str {
    match chain {
        Chain::Bitcoin => "bitcoin",
        Chain::Btcvm => "btcvm",
    }
}

fn parse_chain(name: &str) -> Outcome<Chain> {
    match name {
        "bitcoin" => Ok(Chain::Bitcoin),
        "btcvm" => Ok(Chain::Btcvm),
        _ => fail("unknown network"),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn source_name(source: CheckSource) -> &'static str {
    match source {
        CheckSource::OldPegOnBitcoin => "oldPegOnBitcoin",
        CheckSource::NewPegOnBitcoin => "newPegOnBitcoin",
        CheckSource::BtcvmDocs => "btcvmDocs",
        CheckSource::BtcvmExplorer => "btcvmExplorer",
        CheckSource::RotationProcedure => "rotationProcedure",
        CheckSource::WalletMaker => "walletMaker",
    }
}

impl Wallet {
    pub fn open(language: &str) -> Result<Wallet, String> {
        let dir = btcvm_wallet_vault::default_dir().map_err(|e| e.to_string())?;
        let settings = store::load(&dir);
        let server = settings
            .server
            .clone()
            .unwrap_or_else(|| DEFAULT_SERVER.into());
        let language = settings.language.clone().unwrap_or_else(|| language.into());
        Ok(Wallet {
            vault: Vault::new(&dir, WindowsHello),
            dir,
            settings: Mutex::new(settings),
            bridge: Mutex::new(Arc::new(Bridge::new(&server))),
            snapshot: Mutex::new(Snapshot::default()),
            pending: Mutex::new(None),
            refreshing: Mutex::new(()),
            refresh_again: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
            language: Mutex::new(language),
        })
    }

    pub fn language(&self) -> String {
        self.language.lock().unwrap().clone()
    }

    /// Takes the system's language, unless the user chose one.
    pub fn adopt_system_language(&self, language: &str) {
        if self.settings.lock().unwrap().language.is_none() {
            *self.language.lock().unwrap() = if language.starts_with("es") {
                "es"
            } else {
                "en"
            }
            .into();
        }
    }

    /// The bridge's stream of new blocks.
    pub fn events(&self) -> Result<impl std::io::BufRead + use<>, ApiError> {
        self.bridge().events()
    }

    /// The wallet's address, from the vault file. Shown only; the key is
    /// checked against it every time the vault opens.
    fn address(&self) -> Option<String> {
        self.vault.address().ok()
    }

    fn bridge(&self) -> Arc<Bridge> {
        self.bridge.lock().unwrap().clone()
    }

    fn save(&self, settings: &Settings) {
        if let Err(e) = store::save(&self.dir, settings) {
            eprintln!("saving settings: {e}");
        }
    }

    pub fn view(&self) -> View {
        let settings = self.settings.lock().unwrap().clone();
        let snap = self.snapshot.lock().unwrap();
        let vault_problem = match self.vault.address() {
            Ok(_) => None,
            Err(VaultError::Missing) => None,
            Err(e) => Some(e.to_string()),
        };
        let address = self.address();
        let receive_blocked = settings.backup_required && !settings.backup_confirmed;
        let balance = |v: &Option<AddressView>| {
            v.as_ref().map(|v| Balance {
                confirmed: v.confirmed.clone(),
                pending: v.pending.clone(),
            })
        };
        let mut history: Vec<HistoryRow> = Vec::new();
        for (chain, view) in [("btcvm", &snap.btcvm), ("bitcoin", &snap.bitcoin)] {
            if let Some(v) = view {
                history.extend(v.history.iter().take(25).map(|h| HistoryRow {
                    chain,
                    txid: h.txid.clone(),
                    net: h.net.clone(),
                    confirmations: h.confirmations,
                    time: h.time,
                }));
            }
        }
        history.sort_by_key(|h| std::cmp::Reverse(h.time.unwrap_or(i64::MAX)));
        View {
            version: env!("CARGO_PKG_VERSION"),
            server: self.bridge().base().to_string(),
            language: self.language(),
            has_wallet: self.vault.has_key(),
            vault_problem,
            address: address.filter(|_| !receive_blocked),
            receive_blocked,
            backup_confirmed: settings.backup_confirmed,
            bitcoin: balance(&snap.bitcoin),
            btcvm: balance(&snap.btcvm),
            bitcoin_note: snap.bitcoin_note.clone(),
            history,
            deposits: snap.deposits.clone(),
            withdrawals: settings.withdrawals.clone(),
            in_flight: settings.outgoing.clone(),
            bridge: self.bridge_view(&snap),
            connection_error: snap.connection_error.clone(),
            updated: snap.updated,
        }
    }

    fn bridge_view(&self, snap: &Snapshot) -> BridgeView {
        let Some(info) = &snap.info else {
            return BridgeView::default();
        };
        let status = snap.status.clone().unwrap_or_default();
        let signer_change = snap.bridge.as_ref().and_then(|b| {
            b.signer_change.as_ref().map(|c| SignerChangeView {
                trusted_peg: c.trusted_peg.address(&b.net),
                reported_peg: c.reported_peg.address(&b.net),
                added: c.added_keys(),
                removed: c.removed_keys(),
                links: c
                    .where_to_check(&b.net)
                    .into_iter()
                    .map(|l| LinkView {
                        source: source_name(l.source),
                        url: l.url,
                    })
                    .collect(),
            })
        });
        BridgeView {
            connected: true,
            trusted: snap.bridge.is_some(),
            error: snap.bridge_error.clone(),
            signer_change,
            peg_address: snap.bridge.as_ref().map(|b| b.peg.address(&b.net)),
            min_deposit: snap.bridge.as_ref().map(|b| format_btc(b.min_deposit)),
            max_deposit: snap
                .bridge
                .as_ref()
                .filter(|b| b.max_deposit > 0)
                .map(|b| format_btc(b.max_deposit)),
            min_peg_out: snap.bridge.as_ref().map(|b| format_btc(b.min_peg_out)),
            fee_rate: snap.bridge.as_ref().map(|b| b.btc_fee_rate),
            vm_fee: info.vm_fee.clone(),
            payout_fee: info.payout_fee.clone(),
            paused: status.paused.is_some(),
            solvent: status.audit.as_ref().map(|a| a.solvent),
            locked: status.audit.as_ref().map(|a| a.locked.clone()),
            circulating: status.audit.as_ref().map(|a| a.circulating.clone()),
            bitcoin_syncing: status.bitcoin_sync.is_some_and(|s| s.syncing),
        }
    }

    // --- refreshing ------------------------------------------------------

    /// Refreshes everything from the bridge. Calls that arrive while one is
    /// running fold into a single follow-up pass.
    pub fn refresh(&self) {
        let Ok(_running) = self.refreshing.try_lock() else {
            self.refresh_again.store(true, Ordering::SeqCst);
            return;
        };
        loop {
            self.refresh_again.store(false, Ordering::SeqCst);
            self.refresh_once();
            if !self.refresh_again.swap(false, Ordering::SeqCst) {
                break;
            }
        }
    }

    fn refresh_once(&self) {
        let api = self.bridge();
        let info = match api.info() {
            Ok(info) => info,
            Err(e) => {
                let mut snap = self.snapshot.lock().unwrap();
                snap.connection_error = Some(e.message);
                return;
            }
        };
        let (bridge, bridge_error) = match peg::verify(&info, &Pinned::mainnet()) {
            Ok(b) => (Some(b), None),
            Err(e) => (None, Some(Failure::from(e))),
        };
        let status = api.status().ok();
        let (mut btcvm, mut bitcoin, mut bitcoin_note, mut deposits) =
            (None, None, None, Vec::new());
        if let Some(address) = self.address() {
            btcvm = api.address(Chain::Btcvm, &address).ok();
            if info.btc_wallet {
                match api.address(Chain::Bitcoin, &address) {
                    Ok(v) => bitcoin = Some(v),
                    // First time: register the address with the bridge.
                    Err(e) if e.status == 404 => {
                        let _ = api.watch_bitcoin(&address);
                        bitcoin = api.address(Chain::Bitcoin, &address).ok();
                        if bitcoin.is_none() {
                            bitcoin_note = Some("watching".into());
                        }
                    }
                    Err(e) if e.status == 503 => bitcoin_note = Some("syncing".into()),
                    Err(e) => bitcoin_note = Some(e.message),
                }
            } else {
                bitcoin_note = Some("unavailable".into());
            }
            deposits = api.deposits(&address).unwrap_or_default();
            self.follow_withdrawals(&api);
            self.settle(Chain::Btcvm, btcvm.as_ref());
            self.settle(Chain::Bitcoin, bitcoin.as_ref());
        }
        let mut snap = self.snapshot.lock().unwrap();
        *snap = Snapshot {
            info: Some(info),
            bridge,
            bridge_error,
            status,
            btcvm,
            bitcoin,
            bitcoin_note,
            deposits,
            connection_error: None,
            updated: now(),
        };
    }

    /// Checks each withdrawal not yet confirmed on Bitcoin with the bridge.
    fn follow_withdrawals(&self, api: &Bridge) {
        let open: Vec<String> = self
            .settings
            .lock()
            .unwrap()
            .withdrawals
            .iter()
            .filter(|w| w.status != "paid" || w.payment_confirmations.unwrap_or(0) == 0)
            .map(|w| w.txid.clone())
            .collect();
        for txid in open {
            let Ok(s) = api.peg_out(&txid) else { continue };
            let mut settings = self.settings.lock().unwrap();
            if let Some(w) = settings.withdrawals.iter_mut().find(|w| w.txid == txid) {
                w.status = if s.status == "unknown" && w.status == "sending" {
                    "sending".into()
                } else {
                    s.status
                };
                w.pays = s.pays;
                w.payment_txid = s.payment_txid;
                w.payment_confirmations = s.payment_confirmations;
            }
            let copy = settings.clone();
            drop(settings);
            self.save(&copy);
        }
    }

    /// Forgets payments the chain shows confirmed, or hasn't shown after an
    /// hour (dropped by the network).
    fn settle(&self, chain: Chain, view: Option<&AddressView>) {
        let Some(view) = view else { return };
        let seen: HashMap<&str, i64> = view
            .history
            .iter()
            .map(|h| (h.txid.as_str(), h.confirmations))
            .collect();
        let mut settings = self.settings.lock().unwrap();
        let before = settings.outgoing.len();
        let name = chain_name(chain);
        let time = now();
        settings.outgoing.retain(|o| {
            if o.chain != name {
                return true;
            }
            match seen.get(o.txid.as_str()) {
                Some(&confirmations) => confirmations <= 0,
                None => time.saturating_sub(o.time) < 3600,
            }
        });
        if settings.outgoing.len() != before {
            let copy = settings.clone();
            drop(settings);
            self.save(&copy);
        }
    }

    /// The signer change to warn about once, if one is new.
    pub fn new_signer_change(&self) -> Option<SignerChangeView> {
        let change = {
            let snap = self.snapshot.lock().unwrap();
            self.bridge_view(&snap).signer_change?
        };
        let mut settings = self.settings.lock().unwrap();
        if settings.notified_signer_change.as_deref() == Some(change.reported_peg.as_str()) {
            return None;
        }
        settings.notified_signer_change = Some(change.reported_peg.clone());
        let copy = settings.clone();
        drop(settings);
        self.save(&copy);
        Some(change)
    }

    // --- the key -----------------------------------------------------------

    /// Makes a new wallet. `show_backup` shows the key outside the web view
    /// and says whether the user saved it; until they do, nothing can be
    /// received, as this PC holds the only copy.
    pub fn create(&self, show_backup: impl Fn(&str) -> bool) -> Outcome<View> {
        if self.vault.has_key() {
            return fail("this PC already has a wallet; remove it first");
        }
        let key = Key::generate();
        self.vault.store(&key)?;
        {
            let mut settings = self.settings.lock().unwrap();
            settings.backup_required = true;
            settings.backup_confirmed = false;
            settings.outgoing.clear();
            settings.withdrawals.clear();
            let copy = settings.clone();
            drop(settings);
            self.save(&copy);
        }
        let saved = show_backup(&key.wif(&MAINNET));
        drop(key);
        if saved {
            self.mark_backed_up();
        }
        self.refresh();
        Ok(self.view())
    }

    /// Adopts a key the user already has (their backup), from the clipboard.
    pub fn import(&self, text: Zeroizing<String>) -> Outcome<View> {
        if self.vault.has_key() {
            return fail("this PC already has a wallet; remove it first");
        }
        let key = Key::parse(&text)?;
        drop(text);
        self.vault.store(&key)?;
        {
            let mut settings = self.settings.lock().unwrap();
            settings.outgoing.clear();
            settings.withdrawals.clear();
            let copy = settings.clone();
            drop(settings);
            self.save(&copy);
        }
        // The user brought the key, so they have it.
        self.mark_backed_up();
        self.refresh();
        Ok(self.view())
    }

    /// Shows the key for backup, after Windows Hello.
    pub fn backup(&self, show_backup: impl Fn(&str) -> bool) -> Outcome<View> {
        let key = self.vault.unlock()?;
        if show_backup(&key.wif(&MAINNET)) {
            self.mark_backed_up();
        }
        Ok(self.view())
    }

    /// Puts the wallet back from its backup when the vault can't be opened:
    /// the key must be this wallet's.
    pub fn restore(&self, text: Zeroizing<String>) -> Outcome<View> {
        let key = Key::parse(&text)?;
        drop(text);
        self.vault.restore(&key)?;
        self.mark_backed_up();
        self.refresh();
        Ok(self.view())
    }

    /// Removes the wallet from this PC after Windows Hello. `confirm` asks
    /// the user, outside the web view.
    pub fn remove(&self, confirm: impl Fn() -> bool) -> Outcome<View> {
        if !confirm() {
            return Ok(self.view());
        }
        match self.vault.remove() {
            Ok(()) => {}
            // A vault that can't be opened is set aside instead: the key may
            // still be recoverable from it some other way.
            Err(e) if e.needs_restore() => {
                self.vault.set_aside()?;
            }
            Err(e) => return Err(e.into()),
        }
        {
            let mut settings = self.settings.lock().unwrap();
            let server = settings.server.clone();
            let language = settings.language.clone();
            *settings = Settings {
                server,
                language,
                ..Settings::default()
            };
            let copy = settings.clone();
            drop(settings);
            self.save(&copy);
        }
        *self.pending.lock().unwrap() = None;
        let mut snap = self.snapshot.lock().unwrap();
        snap.btcvm = None;
        snap.bitcoin = None;
        snap.deposits.clear();
        drop(snap);
        Ok(self.view())
    }

    fn mark_backed_up(&self) {
        let mut settings = self.settings.lock().unwrap();
        settings.backup_confirmed = true;
        settings.backup_required = false;
        let copy = settings.clone();
        drop(settings);
        self.save(&copy);
    }

    // --- payments: review, then sign exactly what was reviewed ---------------

    fn verified(&self) -> Outcome<VerifiedBridge> {
        let snap = self.snapshot.lock().unwrap();
        if let Some(e) = &snap.bridge_error {
            return Err(e.clone());
        }
        snap.bridge
            .clone()
            .map_or_else(|| fail("the bridge isn't connected yet"), Ok)
    }

    fn own(&self) -> Outcome<Destination> {
        let address = self
            .address()
            .map_or_else(|| fail("there is no wallet on this PC"), Ok)?;
        Ok(decode_address(&address, &MAINNET)?)
    }

    /// The coins on `chain` this wallet can spend: confirmed, not spent by a
    /// payment still in flight, with the transactions that made them.
    fn coins(
        &self,
        chain: Chain,
    ) -> Outcome<(Vec<btcvm_wallet_core::Utxo>, HashMap<String, String>)> {
        let utxos = {
            let snap = self.snapshot.lock().unwrap();
            let view = match chain {
                Chain::Bitcoin => &snap.bitcoin,
                Chain::Btcvm => &snap.btcvm,
            };
            let Some(view) = view else {
                return fail(format!("your {} balance hasn't loaded yet", chain.name()));
            };
            view.utxos.clone()
        };
        let in_use: HashSet<String> = self
            .settings
            .lock()
            .unwrap()
            .outgoing
            .iter()
            .filter(|o| o.chain == chain_name(chain))
            .flat_map(|o| o.spent.clone())
            .collect();
        let utxos: Vec<_> = utxos
            .into_iter()
            .filter(|u| u.confirmations > 0 && !in_use.contains(&format!("{}:{}", u.txid, u.vout)))
            .collect();
        let api = self.bridge();
        let mut raw = HashMap::new();
        for txid in utxos.iter().map(|u| u.txid.clone()).collect::<HashSet<_>>() {
            let hex = api.raw_tx(chain, &txid)?;
            raw.insert(txid, hex);
        }
        Ok((utxos, raw))
    }

    fn keep(&self, kind: Kind, plan: Plan, to: String, amount: u64) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        *self.pending.lock().unwrap() = Some(Pending {
            id,
            kind,
            plan,
            to,
            amount,
        });
        id
    }

    pub fn prepare_send(&self, chain: &str, to: &str, amount: &str) -> Outcome<Review> {
        let chain = parse_chain(chain)?;
        let bridge = self.verified()?;
        let amount = parse_btc(amount)?;
        let from = self.own()?;
        let (utxos, raw) = self.coins(chain)?;
        let coins = Coins {
            utxos: &utxos,
            raw_txs: &raw,
        };
        let to = to.trim().to_string();
        let plan = plan_send(&bridge, chain, &from, coins, &to, amount)?;
        Ok(self.review(Kind::Send, plan, to, amount, &bridge))
    }

    pub fn prepare_deposit(&self, amount: &str) -> Outcome<Review> {
        let bridge = self.verified()?;
        if bridge.signer_change.is_some() {
            return Err(btcvm_wallet_core::Error::Untrusted(
                btcvm_wallet_core::wallet::PAUSED_BY_SIGNER_CHANGE.into(),
            )
            .into());
        }
        let amount = parse_btc(amount)?;
        let from = self.own()?;
        let address = from.address(&MAINNET);
        // Registers the address with the bridge; the core checks the answer.
        let told = self.bridge().deposit_address(&address)?;
        let (utxos, raw) = self.coins(Chain::Bitcoin)?;
        let coins = Coins {
            utxos: &utxos,
            raw_txs: &raw,
        };
        let plan = plan_deposit(&bridge, &from, coins, amount, &told)?;
        Ok(self.review(Kind::Deposit, plan, told, amount, &bridge))
    }

    pub fn prepare_withdrawal(&self, to: &str, amount: &str) -> Outcome<Review> {
        let bridge = self.verified()?;
        let amount = parse_btc(amount)?;
        let from = self.own()?;
        let (utxos, raw) = self.coins(Chain::Btcvm)?;
        let coins = Coins {
            utxos: &utxos,
            raw_txs: &raw,
        };
        let to = to.trim().to_string();
        let plan = plan_withdrawal(&bridge, &from, coins, amount, &to)?;
        Ok(self.review(Kind::Withdraw, plan, to, amount, &bridge))
    }

    fn review(
        &self,
        kind: Kind,
        plan: Plan,
        to: String,
        amount: u64,
        bridge: &VerifiedBridge,
    ) -> Review {
        let info = self.snapshot.lock().unwrap().info.clone();
        let outputs = plan
            .describe(&MAINNET)
            .into_iter()
            .map(|o| {
                let dest = o
                    .address
                    .as_deref()
                    .and_then(|a| decode_address(a, &MAINNET).ok());
                let role = if o.change {
                    "change"
                } else if o.withdrawal_to.is_some() || o.data.is_some() {
                    "tag"
                } else if dest.as_ref().is_some_and(|d| bridge.is_peg(d)) {
                    "reserve"
                } else if kind == Kind::Deposit && o.address.is_some() {
                    "deposit"
                } else if o.address.is_some() {
                    "pay"
                } else {
                    "other"
                };
                OutputRow {
                    value: format_btc(o.value),
                    address: o.address,
                    role,
                    withdrawal_to: o.withdrawal_to,
                }
            })
            .collect();
        let vm_fee = info
            .as_ref()
            .and_then(|i| i.vm_fee.as_deref())
            .and_then(|f| parse_btc(f).ok());
        let tiers = info.as_ref().map(|i| {
            i.confirmation_tiers
                .iter()
                .find(|t| parse_btc(&t.up_to).is_ok_and(|up| amount <= up))
                .map_or(i.deposit_confirmations, |t| t.confirmations)
        });
        let review = Review {
            id: 0,
            kind: kind.name(),
            chain: chain_name(plan.chain),
            to: to.clone(),
            amount: format_btc(amount),
            fee: format_btc(plan.fee),
            total: format_btc(amount + plan.fee),
            outputs,
            credited: (kind == Kind::Deposit)
                .then(|| vm_fee.map(|f| format_btc(amount.saturating_sub(f))))
                .flatten(),
            confirmations: (kind == Kind::Deposit).then_some(tiers).flatten(),
            payout_fee: (kind == Kind::Withdraw)
                .then(|| info.as_ref().and_then(|i| i.payout_fee.clone()))
                .flatten(),
        };
        let id = self.keep(kind, plan, to, amount);
        Review { id, ..review }
    }

    /// The most an action can move now, as BTC for the amount field: all the
    /// confirmed coins on its chain less the fee, and for a deposit no more
    /// than the bridge accepts. An address not typed yet is sized as the
    /// largest standard output, so the amount fits whatever it turns out to be.
    pub fn max_amount(&self, action: &str, chain: &str, to: &str) -> Outcome<String> {
        let bridge = self.verified()?;
        let from = self.own()?;
        let largest = |kind| Destination::new(kind, &[0u8; 32]).expect("a 32-byte program");
        let typed = |kind| decode_address(to.trim(), &bridge.net).unwrap_or_else(|_| largest(kind));
        let paused = || -> Outcome<()> {
            if bridge.signer_change.is_some() {
                return Err(btcvm_wallet_core::Error::Untrusted(
                    btcvm_wallet_core::wallet::PAUSED_BY_SIGNER_CHANGE.into(),
                )
                .into());
            }
            Ok(())
        };
        let (chain, dest, data, floor, cap) = match action {
            "send" => (parse_chain(chain)?, typed(AddressKind::P2wsh), None, 0, 0),
            "deposit" => {
                paused()?;
                (
                    Chain::Bitcoin,
                    bridge.signers.deposit_destination(&from)?,
                    None,
                    bridge.min_deposit,
                    bridge.max_deposit,
                )
            }
            "withdraw" => {
                paused()?;
                let tag = peg_out_data(&typed(AddressKind::P2tr));
                (
                    Chain::Btcvm,
                    bridge.peg.clone(),
                    Some(tag),
                    bridge.min_peg_out,
                    0,
                )
            }
            _ => return fail("unknown action"),
        };
        let (utxos, raw) = self.coins(chain)?;
        let max = max_payment(
            &from,
            &utxos,
            &raw,
            &Request {
                chain,
                to: dest,
                amount: 0,
                data,
                fee_rate: bridge.btc_fee_rate,
            },
        )?;
        if max < floor {
            return fail(format!(
                "after the fee there are {} BTC, under the minimum of {} BTC",
                format_btc(max),
                format_btc(floor)
            ));
        }
        Ok(format_btc(if cap > 0 { max.min(cap) } else { max }))
    }

    pub fn cancel(&self, id: u64) {
        let mut pending = self.pending.lock().unwrap();
        if pending.as_ref().is_some_and(|p| p.id == id) {
            *pending = None;
        }
    }

    /// Signs the reviewed payment after Windows Hello and sends it.
    pub fn confirm(&self, id: u64) -> Outcome<Sent> {
        // Kept until signed, so canceling Windows Hello lets the user retry.
        let pending = match self.pending.lock().unwrap().as_ref() {
            Some(p) if p.id == id => p.clone(),
            _ => return fail("that payment is no longer waiting; prepare it again"),
        };
        let own = self.own()?;
        let key = self.vault.unlock()?;
        if key.destination() != own {
            return fail("the key in the vault isn't this wallet's address; nothing was signed");
        }
        let signed = sign_plan(&key, &pending.plan)?;
        drop(key);
        self.cancel(id);
        let raw = hex::decode(&signed.hex).map_err(|_| Failure {
            message: "the signed transaction isn't hex".into(),
            untrusted: false,
            canceled: false,
        })?;
        let parsed = parse_tx(&raw)?;
        if parsed.outputs != pending.plan.outputs() || parsed.txid() != signed.txid {
            return fail(
                "the signed transaction didn't match what you reviewed, so it wasn't sent",
            );
        }
        let chain = pending.plan.chain;
        let own_script = own.pk_script();
        let change: u64 = parsed
            .outputs
            .iter()
            .filter(|o| o.script == own_script)
            .map(|o| o.value)
            .sum();
        let time = now();
        // Recorded before sending, so it's tracked even if the answer is lost.
        {
            let mut settings = self.settings.lock().unwrap();
            settings.outgoing.retain(|o| o.txid != signed.txid);
            settings.outgoing.insert(
                0,
                Outgoing {
                    txid: signed.txid.clone(),
                    chain: chain_name(chain).into(),
                    kind: pending.kind.name().into(),
                    to: pending.to.clone(),
                    amount: pending.amount,
                    change,
                    spent: pending.plan.spends(),
                    time,
                },
            );
            settings.outgoing.truncate(50);
            if pending.kind == Kind::Withdraw {
                settings.withdrawals.insert(
                    0,
                    Withdrawal {
                        txid: signed.txid.clone(),
                        to: pending.to.clone(),
                        amount: pending.amount,
                        time,
                        status: "sending".into(),
                        pays: None,
                        payment_txid: None,
                        payment_confirmations: None,
                    },
                );
                settings.withdrawals.truncate(50);
            }
            let copy = settings.clone();
            drop(settings);
            self.save(&copy);
        }
        match self.bridge().broadcast(chain, &signed.hex) {
            Ok(txid) if txid == signed.txid => Ok(Sent {
                txid,
                chain: chain_name(chain),
            }),
            Ok(txid) => Err(Failure {
                message: format!(
                    "the bridge reported transaction {txid}, but this wallet signed {}",
                    signed.txid
                ),
                untrusted: true,
                canceled: false,
            }),
            Err(e) if e.status == 400 => {
                // Refused, so nothing was sent: its coins are free again.
                let mut settings = self.settings.lock().unwrap();
                settings.outgoing.retain(|o| o.txid != signed.txid);
                settings.withdrawals.retain(|w| w.txid != signed.txid);
                let copy = settings.clone();
                drop(settings);
                self.save(&copy);
                Err(e.into())
            }
            Err(e) => fail(format!(
                "{}. It may or may not have been sent: it stays in the list of payments in flight, \
                 and its coins aren't offered again until it settles. Its id is {}.",
                e.message, signed.txid
            )),
        }
    }

    // --- settings and links ----------------------------------------------------

    pub fn set_server(&self, url: &str) -> Outcome<View> {
        let url = url.trim().trim_end_matches('/');
        let url = if url.is_empty() { DEFAULT_SERVER } else { url };
        let valid = url.strip_prefix("https://").is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-:/".contains(&c))
        });
        if !valid {
            return fail("the bridge's address must be https://, like https://metalbtc.com");
        }
        *self.bridge.lock().unwrap() = Arc::new(Bridge::new(url));
        *self.snapshot.lock().unwrap() = Snapshot::default();
        *self.pending.lock().unwrap() = None;
        {
            let mut settings = self.settings.lock().unwrap();
            settings.server = (url != DEFAULT_SERVER).then(|| url.to_string());
            let copy = settings.clone();
            drop(settings);
            self.save(&copy);
        }
        self.refresh();
        Ok(self.view())
    }

    pub fn set_language(&self, language: &str) -> View {
        let language = if language == "es" { "es" } else { "en" };
        *self.language.lock().unwrap() = language.into();
        let mut settings = self.settings.lock().unwrap();
        settings.language = Some(language.into());
        let copy = settings.clone();
        drop(settings);
        self.save(&copy);
        self.view()
    }

    /// The URL for a link the window asks to open. Only known destinations,
    /// built here: the web view never names a URL itself.
    pub fn link(&self, kind: &str, arg: &str) -> Outcome<String> {
        let txid = || {
            if arg.len() == 64 && arg.bytes().all(|c| c.is_ascii_hexdigit()) {
                Ok(arg.to_ascii_lowercase())
            } else {
                fail("not a transaction id")
            }
        };
        let address = || Ok::<_, Failure>(decode_address(arg, &MAINNET)?.address(&MAINNET));
        Ok(match kind {
            "website" => about::WEBSITE.into(),
            "vote" => about::XPR_VOTE_URL.into(),
            "metal" => about::METAL_VALIDATOR_URL.into(),
            "x" => about::X_URL.into(),
            "btcvm" => "https://metalbtc.com".into(),
            "tx-bitcoin" => format!("https://mempool.space/tx/{}", txid()?),
            "tx-btcvm" => format!("https://metalbtc.com/explorer#/tx/{}", txid()?),
            "address-bitcoin" => format!("https://mempool.space/address/{}", address()?),
            "address-btcvm" => format!("https://metalbtc.com/explorer#/address/{}", address()?),
            "check" => {
                let snap = self.snapshot.lock().unwrap();
                let links = self
                    .bridge_view(&snap)
                    .signer_change
                    .map(|c| c.links)
                    .unwrap_or_default();
                let index: usize = arg.parse().map_err(|_| Failure {
                    message: "no such link".into(),
                    untrusted: false,
                    canceled: false,
                })?;
                match links.get(index) {
                    Some(l) => l.url.clone(),
                    None => return fail("no such link"),
                }
            }
            _ => return fail("no such link"),
        })
    }
}
