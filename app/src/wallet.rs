//! The wallets behind the window: what it shows and what it can do.
//!
//! Each wallet is its own key, with its own vault, Windows Hello key and
//! backup, and an address for each coin: one for BTC, the same on Bitcoin and
//! BTCVM, and one for DOGE, the same on Dogecoin and DogecoinVM. One wallet
//! and one coin are shown at a time; each coin has its own bridge, kept on
//! its own side. Every payment is planned by the core for review and kept here;
//! the web view gets a description and an id, never a key or anything it
//! could change before signing. Signing asks for Windows Hello, reads the
//! signed transaction back, and records the payment before it is sent.

use crate::api::{
    AddressView, Api, ApiError, Bridge, BridgeStatus, DEFAULT_SERVER, DOGE_SERVER, DepositEntry,
    FeeEstimates, Prices,
};
use crate::journal;
use crate::store::{self, FIRST, Outgoing, RotationProof, Settings, WalletRecord, Withdrawal};
use btcvm_wallet_core::book::{self, Contact};
use btcvm_wallet_core::bridge::{
    self as peg, CheckSource, ConfirmationTier, Pinned, SignerChange, Signers, peg_out_data_for,
};
use btcvm_wallet_core::encoding::sha256;
use btcvm_wallet_core::payment::MAX_FEE_RATE;
use btcvm_wallet_core::rotation;
use btcvm_wallet_core::tx::parse_tx;
use btcvm_wallet_core::{
    BridgeInfo, Chain, Coin, Coins, DOGE_MAINNET, Destination, DogeBridgeInfo, Kind as AddressKind,
    MAINNET, Plan, Request, VerifiedBridge, about, decode_address, format_btc, max_payment,
    plan_bump, plan_deposit, plan_send, plan_withdrawal, sign_plan,
};
use btcvm_wallet_vault::{Gate, Secret, Vault, VaultError, WindowsHello};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
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

/// Locks a mutex even after a thread panicked holding it. What the wallet
/// keeps behind them is plain state, saved whole or fetched again on the next
/// refresh, so going on beats every later lock failing too.
trait Guard<T> {
    fn guard(&self) -> MutexGuard<'_, T>;
}

impl<T> Guard<T> for Mutex<T> {
    fn guard(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn paused(coin: Coin) -> Failure {
    btcvm_wallet_core::Error::Untrusted(btcvm_wallet_core::wallet::paused_by_signer_change(coin))
        .into()
}

/// What the window and the reviews use of a bridge's `/api/info`, whichever
/// coin's bridge it is.
#[derive(Clone, Default)]
struct Facts {
    /// Whether the bridge serves balances on the coin's own chain too.
    l1_wallet: bool,
    /// What it keeps from a deposit, in the coin.
    vm_fee: Option<String>,
    /// What a withdrawal's payout costs on the coin's own chain, taken from
    /// the amount paid.
    payout_fee: Option<String>,
    confirmation_tiers: Vec<ConfirmationTier>,
    deposit_confirmations: u32,
}

impl From<&BridgeInfo> for Facts {
    fn from(i: &BridgeInfo) -> Facts {
        Facts {
            l1_wallet: i.btc_wallet,
            vm_fee: i.vm_fee.clone(),
            payout_fee: i.payout_fee.clone(),
            confirmation_tiers: i.confirmation_tiers.clone(),
            deposit_confirmations: i.deposit_confirmations,
        }
    }
}

impl From<&DogeBridgeInfo> for Facts {
    fn from(i: &DogeBridgeInfo) -> Facts {
        Facts {
            l1_wallet: i.doge_wallet,
            vm_fee: i.vm_fee.clone(),
            payout_fee: i.doge_fee.clone(),
            confirmation_tiers: i.confirmation_tiers.clone(),
            deposit_confirmations: i.deposit_confirmations,
        }
    }
}

/// What a side last fetched from its bridge.
#[derive(Default)]
struct Snapshot {
    info: Option<Facts>,
    bridge: Option<VerifiedBridge>,
    bridge_error: Option<Failure>,
    status: Option<BridgeStatus>,
    /// The wallet the views below belong to.
    wallet: Option<String>,
    /// Its address on the coin's VM, and on the coin's own chain.
    vm: Option<AddressView>,
    l1: Option<AddressView>,
    l1_note: Option<String>,
    /// mempool.space's confirmed balance for the active wallet, sats, when
    /// asked and it differs from the bridge's. BTC only.
    second_opinion: Option<u64>,
    deposits: Vec<DepositEntry>,
    /// Every wallet's balances, by id: the coin's own chain, then its VM.
    balances: HashMap<String, (Option<Balance>, Option<Balance>)>,
    connection_error: Option<String>,
    updated: u64,
}

/// What the wallet keeps for one coin: its bridge, the signer set it trusts
/// there, what it last fetched, and the coin's price.
struct Side {
    coin: Coin,
    bridge: Mutex<Arc<dyn Api>>,
    /// The signer set built into the wallet, where trust starts.
    built_in: Pinned,
    /// The peg's signer set the wallet trusts: the one built in, or for BTC
    /// the last it followed by a checked rotation.
    trusted: Mutex<Signers>,
    /// When the wallet last looked for the move behind a signer change.
    rotation_checked: Mutex<u64>,
    snapshot: Mutex<Snapshot>,
    /// The coin's price, and when it was fetched.
    prices: Mutex<Option<(Prices, u64)>>,
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
    wallet: String,
    kind: Kind,
    plan: Plan,
    to: String,
    amount: u64,
    /// For a replacement with a higher fee: the payment it replaces.
    replaces: Option<String>,
}

/// What unlocks the wallets' vaults: Windows Hello, or in tests a stand-in.
pub type SharedGate = Arc<dyn Gate + Send + Sync>;

/// Makes the API for a bridge's address: the network, or in tests a
/// stand-in.
pub type Connect = Box<dyn Fn(&str) -> Arc<dyn Api> + Send + Sync>;

pub struct Wallet {
    dir: PathBuf,
    settings: Mutex<Settings>,
    connect: Connect,
    gate: SharedGate,
    /// BTC's side, then DOGE's.
    sides: [Side; 2],
    pending: Mutex<Option<Pending>>,
    refreshing: Mutex<()>,
    refresh_again: AtomicBool,
    next_id: AtomicU64,
    language: Mutex<String>,
    /// The active wallet's transactions seen so far, to tell what's new.
    seen: Mutex<Seen>,
    /// Payments received, for the window to announce once.
    notices: Mutex<Vec<Notice>>,
    /// Wallets whose key Windows was asked to certify since the app started.
    attestation_asked: Mutex<HashSet<String>>,
    /// What the user is to type back from the backup just shown.
    backup_check: Mutex<Option<BackupCheck>>,
}

/// A wallet's backup, to show outside the web view: its words, when it has
/// a recovery phrase, and its key as a WIF.
pub struct BackupText {
    pub words: Option<Zeroizing<String>>,
    pub wif: Zeroizing<String>,
}

impl BackupText {
    fn of(secret: &Secret) -> Outcome<BackupText> {
        Ok(BackupText {
            words: secret.words(),
            wif: secret.key()?.wif(&MAINNET),
        })
    }
}

/// Some words of a backup just shown, or some of its key's characters, for
/// the user to type back. Kept here: the window learns only the positions.
struct BackupCheck {
    wallet: String,
    words: bool,
    /// 1-based, first and last.
    items: Vec<(usize, usize)>,
    expected: Zeroizing<Vec<String>>,
}

#[derive(Serialize, Clone)]
pub struct BackupCheckView {
    /// "words" or "chars".
    pub kind: &'static str,
    /// 1-based, first and last.
    pub items: Vec<[usize; 2]>,
}

#[derive(Default)]
struct Seen {
    wallet: Option<String>,
    /// The chains whose history was read for that wallet.
    chains: HashSet<&'static str>,
    /// chain:txid.
    txids: HashSet<String>,
}

/// A payment that arrived: on which chain, how much, and whether it's
/// confirmed yet.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub chain: &'static str,
    pub amount: String,
    pub confirmed: bool,
}

/// The active wallet's address as a QR code: its modules row by row, "1"
/// for dark.
#[derive(Serialize)]
pub struct Qr {
    pub width: usize,
    pub modules: String,
    pub address: String,
}

/// A currency and the coin shown's price in it.
#[derive(Serialize, Clone)]
pub struct Fiat {
    pub currency: &'static str,
    pub price: f64,
}

// --- what the window shows ---------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub version: &'static str,
    pub server: String,
    pub language: String,
    pub has_wallet: bool,
    pub wallets: Vec<WalletView>,
    pub wallet_id: Option<String>,
    pub wallet_name: String,
    /// Set when the active wallet's vault can't be opened and only the
    /// backup can help.
    pub vault_problem: Option<String>,
    /// The coin shown: "btc" or "doge".
    pub coin: &'static str,
    /// Each coin at a glance, for the total and the coin selector.
    pub coins: Vec<CoinSummary>,
    /// The active wallet's address for the coin shown. Hidden while a new
    /// wallet's key isn't backed up.
    pub address: Option<String>,
    /// A phrase wallet's DOGE address isn't known until the phrase is
    /// unlocked once with Windows Hello.
    pub address_unknown: bool,
    pub receive_blocked: bool,
    pub backup_confirmed: bool,
    /// The coin shown on its own chain (Bitcoin, Dogecoin) and on its VM.
    pub l1: Option<Balance>,
    pub vm: Option<Balance>,
    pub l1_note: Option<String>,
    pub history: Vec<HistoryRow>,
    pub deposits: Vec<DepositEntry>,
    pub withdrawals: Vec<Withdrawal>,
    pub in_flight: Vec<Outgoing>,
    pub address_book: Vec<Contact>,
    pub bridge: BridgeView,
    pub connection_error: Option<String>,
    /// What the deposits not yet credited will add on the VM, after the
    /// bridge's fee: pending there until the bridge credits them.
    pub vm_incoming: Option<String>,
    /// An unexpected failure since the app started, logged in detail.
    pub internal_error: Option<String>,
    /// What the user chose to see values in: "EUR", "USD" or "none".
    pub fiat_choice: String,
    /// The coin shown's price in that currency, once known.
    pub fiat: Option<Fiat>,
    /// Whether Windows certified the active wallet's Windows Hello key to be
    /// in a TPM.
    pub hardware: Option<bool>,
    /// Windows can't certify it, and the user hasn't seen that yet.
    pub software_key_warning: bool,
    /// The positions to type back from the active wallet's backup.
    pub backup_check: Option<BackupCheckView>,
    /// Whether the Bitcoin balance is checked with mempool.space too.
    pub check_balances: bool,
    /// Whether updates are looked for, and whether this build can update
    /// itself at all.
    pub updates_on: bool,
    pub updates_possible: bool,
    /// mempool.space's confirmed Bitcoin balance, BTC, when it differs from
    /// the bridge's.
    pub l1_disagrees: Option<String>,
    pub updated: u64,
}

/// One coin at a glance: what the active wallet holds on its two chains,
/// its price, and whether its bridge needs a look.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoinSummary {
    pub coin: &'static str,
    pub l1: Option<Balance>,
    pub vm: Option<Balance>,
    /// The coin's price in the chosen currency, once known.
    pub price: Option<f64>,
    /// The bridge can't be reached or checked, or reports another signer set.
    pub attention: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WalletView {
    pub id: String,
    /// Empty for the window's default name.
    pub name: String,
    /// Hidden while its key isn't backed up.
    pub address: Option<String>,
    pub active: bool,
    pub needs_backup: bool,
    /// The coin shown, on its own chain and on its VM.
    pub l1: Option<Balance>,
    pub vm: Option<Balance>,
    pub hardware: Option<bool>,
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
    pub coin: &'static str,
    /// The bridge's server.
    pub server: String,
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
    /// The bridge's node of the coin's own chain is catching up.
    pub l1_syncing: bool,
    /// The last rotation the wallet followed, until the user has seen it.
    pub rotation: Option<RotationView>,
}

/// A rotation the wallet checked by itself.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RotationView {
    pub from_peg: String,
    pub to_peg: String,
    pub chain: String,
    pub txid: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignerChangeView {
    /// Whose bridge: "btc" or "doge".
    pub coin: &'static str,
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

/// The Bitcoin fee rates to offer, sat/vB: the bridge's estimate, and
/// mempool.space's when it answers.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeOptions {
    pub bridge: Option<u64>,
    pub fastest: Option<u64>,
    pub half_hour: Option<u64>,
    pub hour: Option<u64>,
    pub economy: Option<u64>,
    pub minimum: Option<u64>,
    pub max: u64,
}

/// A payment for review: everything it does, in plain terms.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub id: u64,
    pub kind: &'static str,
    pub chain: &'static str,
    /// The coin's ticker, for the amounts: "BTC" or "DOGE".
    pub ticker: &'static str,
    pub wallet_name: String,
    /// Who is paid: the address typed, the deposit address, or for a
    /// withdrawal the Bitcoin address the bridge pays.
    pub to: String,
    /// What the wallet knows of `to`, and the name it knows it by.
    pub to_known: &'static str,
    pub to_name: Option<String>,
    /// A wallet or contact `to` looks like without being it.
    pub lookalike: Option<String>,
    pub amount: String,
    pub fee: String,
    /// sat/vB, on Bitcoin.
    pub fee_rate: Option<u64>,
    /// What leaves the wallet: the amount and the fee.
    pub total: String,
    pub outputs: Vec<OutputRow>,
    /// A deposit: what the VM credits, and after how many confirmations.
    pub credited: Option<String>,
    pub confirmations: Option<u32>,
    /// A withdrawal: the coin's own chain's fee taken from the payout at
    /// today's rate.
    pub payout_fee: Option<String>,
    /// A replacement: the fee of the payment it replaces.
    pub previous_fee: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutputRow {
    pub value: String,
    pub address: Option<String>,
    /// pay, deposit, reserve, change, tag or other.
    pub role: &'static str,
    pub withdrawal_to: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct Sent {
    pub txid: String,
    pub chain: &'static str,
}

fn chain_name(chain: Chain) -> &'static str {
    match chain {
        Chain::Bitcoin => "bitcoin",
        Chain::Btcvm => "btcvm",
        Chain::Dogecoin => "dogecoin",
        Chain::Dogecoinvm => "dogecoinvm",
    }
}

/// How a withdrawal's record names its coin: empty for BTC, as before DOGE.
fn coin_name(coin: Coin) -> String {
    match coin {
        Coin::Btc => String::new(),
        Coin::Doge => "doge".into(),
    }
}

/// A phrase wallet's DOGE address, from its secret: its DOGE key is another
/// than its BTC one. A key's follows from its BTC address, so it isn't kept.
fn phrase_doge_address(secret: &Secret) -> Option<String> {
    match secret {
        Secret::Phrase(_) => secret
            .key_for(Coin::Doge)
            .ok()
            .map(|k| k.p2pkh_destination().address(&DOGE_MAINNET)),
        Secret::Key(_) => None,
    }
}

fn parse_chain(name: &str) -> Outcome<Chain> {
    match name {
        "bitcoin" => Ok(Chain::Bitcoin),
        "btcvm" => Ok(Chain::Btcvm),
        "dogecoin" => Ok(Chain::Dogecoin),
        "dogecoinvm" => Ok(Chain::Dogecoinvm),
        _ => fail("unknown network"),
    }
}

/// A coin as the window and the settings name it.
fn coin_key(coin: Coin) -> &'static str {
    match coin {
        Coin::Btc => "btc",
        Coin::Doge => "doge",
    }
}

fn parse_coin(name: &str) -> Outcome<Coin> {
    match name {
        "btc" => Ok(Coin::Btc),
        "doge" => Ok(Coin::Doge),
        _ => fail("unknown coin"),
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
        CheckSource::OldPegOnDogecoin => "oldPegOnDogecoin",
        CheckSource::NewPegOnDogecoin => "newPegOnDogecoin",
        CheckSource::DogecoinvmDocs => "dogecoinvmDocs",
        CheckSource::DogecoinvmExplorer => "dogecoinvmExplorer",
        CheckSource::DogecoinvmSigners => "dogecoinvmSigners",
    }
}

fn balance(v: Option<&AddressView>) -> Option<Balance> {
    v.map(|v| Balance {
        confirmed: v.confirmed.clone(),
        pending: v.pending.clone(),
    })
}

/// The signer set to trust: the one built into the wallet, followed through
/// each stored rotation that still checks out. A proof that doesn't (edited,
/// or for another lineage) stops the walk there.
fn replay(built_in: &Signers, rotations: &[RotationProof]) -> Signers {
    let mut current = built_in.clone();
    for p in rotations {
        let prev: HashMap<String, String> = p.prev.clone().into_iter().collect();
        let checked = p.from == current
            && hex::decode(&p.tx)
                .ok()
                .is_some_and(|raw| rotation::verify_move(&current, &p.to, &raw, &prev).is_ok());
        if !checked {
            journal::warn("a stored rotation of the signers doesn't check out; it is ignored");
            break;
        }
        current = p.to.clone();
    }
    current
}

fn rotation_txid(p: &RotationProof) -> String {
    hex::decode(&p.tx)
        .ok()
        .and_then(|raw| btcvm_wallet_core::tx::txid(&raw).ok())
        .unwrap_or_default()
}

/// The currency to show values in: the user's choice, or by language.
fn fiat_choice(settings: &Settings, language: &str) -> &'static str {
    match settings.fiat.as_deref() {
        Some("EUR") => "EUR",
        Some("USD") => "USD",
        Some("none") => "none",
        _ if language == "es" => "EUR",
        _ => "USD",
    }
}

/// What deposits on their way will add on the VM: those confirming, waiting
/// for the bridge's capacity, or being credited without a credit yet. Each
/// counts as the bridge says it will credit it, or its amount less the
/// bridge's fee.
fn incoming(deposits: &[DepositEntry], info: Option<&Facts>, coin: Coin) -> Option<String> {
    let parse = |s: &str| coin.parse_amount(s).ok();
    let fee = info
        .and_then(|i| i.vm_fee.as_deref())
        .and_then(parse)
        .unwrap_or(0);
    let total: u64 = deposits
        .iter()
        .filter(|d| {
            matches!(
                d.status.as_str(),
                "confirming" | "waiting_for_capacity" | "crediting"
            ) && d.credit_txid.is_none()
        })
        .map(|d| match d.credited.as_deref().and_then(parse) {
            Some(credited) => credited,
            None => parse(&d.amount).unwrap_or(0).saturating_sub(fee),
        })
        .sum();
    (total > 0).then(|| coin.format_amount(total))
}

/// A wallet's name as stored: cleaned like an address book name, or empty
/// for the default.
fn wallet_name(name: &str) -> Outcome<String> {
    if name.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(book::clean_name(name)?)
}

impl Wallet {
    pub fn open(language: &str) -> Result<Wallet, String> {
        let dir = btcvm_wallet_vault::default_dir().map_err(|e| e.to_string())?;
        Ok(Wallet::open_with(
            dir,
            language,
            Box::new(|url| Arc::new(Bridge::new(url))),
            Arc::new(WindowsHello),
            Pinned::mainnet(),
        ))
    }

    /// The wallets kept in `dir`, reaching servers through `connect`, their
    /// vaults unlocked through `gate`, trusting `built_in` signers first for
    /// BTC, and DogecoinVM's mainnet set for DOGE.
    pub fn open_with(
        dir: PathBuf,
        language: &str,
        connect: Connect,
        gate: SharedGate,
        built_in: Pinned,
    ) -> Wallet {
        Wallet::open_with_pins(
            dir,
            language,
            connect,
            gate,
            built_in,
            Pinned::doge_mainnet(),
        )
    }

    /// As [`Wallet::open_with`], trusting `doge` signers first for DOGE.
    pub fn open_with_pins(
        dir: PathBuf,
        language: &str,
        connect: Connect,
        gate: SharedGate,
        btc: Pinned,
        doge: Pinned,
    ) -> Wallet {
        let mut settings = store::load(&dir);
        // The book is checked again each time it's read.
        settings.address_book = book::parse(&settings.address_book);
        let server = settings
            .server
            .clone()
            .unwrap_or_else(|| DEFAULT_SERVER.into());
        let language = settings.language.clone().unwrap_or_else(|| language.into());
        // BTC follows checked rotations of its signers; DogecoinVM's bridge
        // can't hand over to a new set yet.
        let trusted = replay(&btc.signers, &settings.rotations);
        let side = |coin, url: &str, built_in: Pinned, trusted: Signers| Side {
            coin,
            bridge: Mutex::new(connect(url)),
            built_in,
            trusted: Mutex::new(trusted),
            rotation_checked: Mutex::new(0),
            snapshot: Mutex::new(Snapshot::default()),
            prices: Mutex::new(None),
        };
        let doge_signers = doge.signers.clone();
        let sides = [
            side(Coin::Btc, &server, btc, trusted),
            side(Coin::Doge, DOGE_SERVER, doge, doge_signers),
        ];
        let wallet = Wallet {
            dir,
            settings: Mutex::new(settings),
            connect,
            gate,
            sides,
            pending: Mutex::new(None),
            refreshing: Mutex::new(()),
            refresh_again: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
            language: Mutex::new(language),
            seen: Mutex::new(Seen::default()),
            notices: Mutex::new(Vec::new()),
            attestation_asked: Mutex::new(HashSet::new()),
            backup_check: Mutex::new(None),
        };
        // Saves a single-wallet install in the new form at once.
        let settings = wallet.settings.guard().clone();
        wallet.save(&settings);
        wallet
    }

    pub fn language(&self) -> String {
        self.language.guard().clone()
    }

    /// Where the wallets live, for tests that reopen them.
    #[cfg(test)]
    pub fn data_dir(&self) -> PathBuf {
        self.dir.clone()
    }

    /// Takes the system's language, unless the user chose one.
    pub fn adopt_system_language(&self, language: &str) {
        if self.settings.guard().language.is_none() {
            *self.language.guard() = if language.starts_with("es") {
                "es"
            } else {
                "en"
            }
            .into();
        }
    }

    /// `coin`'s bridge's stream of new blocks.
    pub fn events(&self, coin: Coin) -> Result<Box<dyn std::io::BufRead + Send>, ApiError> {
        self.bridge(coin).events()
    }

    fn side(&self, coin: Coin) -> &Side {
        let side = match coin {
            Coin::Btc => &self.sides[0],
            Coin::Doge => &self.sides[1],
        };
        debug_assert_eq!(side.coin, coin);
        side
    }

    fn bridge(&self, coin: Coin) -> Arc<dyn Api> {
        self.side(coin).bridge.guard().clone()
    }

    /// The coin shown.
    fn active_coin(&self) -> Coin {
        match self.settings.guard().coin.as_deref() {
            Some("doge") => Coin::Doge,
            _ => Coin::Btc,
        }
    }

    fn save(&self, settings: &Settings) {
        if let Err(e) = store::save(&self.dir, settings) {
            eprintln!("saving settings: {e}");
        }
    }

    /// Changes the settings and saves them.
    fn update<R>(&self, f: impl FnOnce(&mut Settings) -> R) -> R {
        let mut settings = self.settings.guard();
        let result = f(&mut settings);
        let copy = settings.clone();
        drop(settings);
        self.save(&copy);
        result
    }

    fn active_id(&self) -> Option<String> {
        self.settings.guard().active.clone()
    }

    fn vault(&self, id: &str) -> Vault<SharedGate> {
        Vault::new(store::vault_dir(&self.dir, id), self.gate.clone())
    }

    /// A wallet's BTC address, from its vault file. Shown only; the key is
    /// checked against it every time the vault opens.
    fn address_of(&self, id: &str) -> Option<String> {
        self.vault(id).address().ok()
    }

    /// A wallet's address for `coin`: BTC's from its vault file, and DOGE's
    /// from it too for a key, or as learned from its phrase. Shown only:
    /// signing checks it against the key.
    fn address_for(&self, id: &str, coin: Coin) -> Option<String> {
        match coin {
            Coin::Btc => self.address_of(id),
            Coin::Doge => match self.vault(id).address_for(Coin::Doge) {
                Ok(Some(address)) => Some(address),
                Ok(None) => self.record(id).and_then(|r| r.doge_address),
                Err(_) => None,
            },
        }
    }

    fn record(&self, id: &str) -> Option<WalletRecord> {
        self.settings
            .lock()
            .unwrap()
            .wallets
            .iter()
            .find(|w| w.id == id)
            .cloned()
    }

    /// A wallet's name for the user: its own, or the default the window
    /// shows for it.
    fn display_name(&self, record: &WalletRecord) -> String {
        if !record.name.is_empty() {
            return record.name.clone();
        }
        let es = self.language() == "es";
        if record.id == FIRST {
            (if es { "Principal" } else { "Main" }).into()
        } else {
            let id = record.id.get(..4).unwrap_or(&record.id);
            format!("{} {id}", if es { "Cartera" } else { "Wallet" })
        }
    }

    /// The active wallet as the user knows it, for native dialogs: its name
    /// and address.
    pub fn active_label(&self) -> String {
        let Some(id) = self.active_id() else {
            return String::new();
        };
        let name = self
            .record(&id)
            .map(|r| self.display_name(&r))
            .unwrap_or_default();
        match self.address_of(&id) {
            Some(address) => format!("{name} ({address})"),
            None => name,
        }
    }

    /// Changes the record of wallet `id`, if it still exists, and saves.
    fn update_record(&self, id: &str, f: impl FnOnce(&mut WalletRecord)) {
        self.update(|s| {
            if let Some(w) = s.wallets.iter_mut().find(|w| w.id == id) {
                f(w);
            }
        });
    }

    pub fn view(&self) -> View {
        let settings = self.settings.guard().clone();
        let coin = self.active_coin();
        let active = settings.active.clone();
        let fiat_choice = fiat_choice(&settings, &self.language());
        // Each coin at a glance, one snapshot at a time: holding two at once
        // could deadlock with a view for the other coin taken meanwhile.
        let coins = Coin::ALL
            .iter()
            .map(|&c| {
                let snap = self.side(c).snapshot.guard();
                self.summary(c, &snap, active.as_ref(), fiat_choice)
            })
            .collect();
        let snap = self.side(coin).snapshot.guard();
        let record = active
            .as_ref()
            .and_then(|id| settings.wallets.iter().find(|w| &w.id == id))
            .cloned()
            .unwrap_or_default();
        let vault_problem = active
            .as_ref()
            .and_then(|id| match self.vault(id).address() {
                Ok(_) => None,
                Err(e) => Some(e.to_string()),
            });
        let receive_blocked = record.backup_required && !record.backup_confirmed;
        let fresh = snap.wallet == active;
        let mut history: Vec<HistoryRow> = Vec::new();
        if fresh {
            for (chain, view) in [
                (chain_name(coin.vm()), &snap.vm),
                (chain_name(coin.l1()), &snap.l1),
            ] {
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
        }
        history.sort_by_key(|h| std::cmp::Reverse(h.time.unwrap_or(i64::MAX)));
        let wallets = settings
            .wallets
            .iter()
            .map(|w| {
                let needs_backup = w.backup_required && !w.backup_confirmed;
                let (l1, vm) = snap.balances.get(&w.id).cloned().unwrap_or((None, None));
                WalletView {
                    id: w.id.clone(),
                    name: w.name.clone(),
                    address: self.address_for(&w.id, coin).filter(|_| !needs_backup),
                    active: Some(&w.id) == active.as_ref(),
                    needs_backup,
                    l1,
                    vm,
                    hardware: w.hardware,
                }
            })
            .collect();
        let fiat = self.price(coin, fiat_choice).map(|price| Fiat {
            currency: fiat_choice,
            price,
        });
        let address = active.as_ref().and_then(|id| self.address_for(id, coin));
        View {
            version: env!("CARGO_PKG_VERSION"),
            server: self.bridge(Coin::Btc).base().to_string(),
            language: self.language(),
            has_wallet: !settings.wallets.is_empty(),
            wallets,
            wallet_id: active.clone(),
            wallet_name: record.name.clone(),
            address_unknown: active.is_some() && vault_problem.is_none() && address.is_none(),
            vault_problem,
            coin: coin_key(coin),
            coins,
            address: address.filter(|_| !receive_blocked),
            receive_blocked,
            backup_confirmed: record.backup_confirmed,
            l1: if fresh {
                balance(snap.l1.as_ref())
            } else {
                None
            },
            vm: if fresh {
                balance(snap.vm.as_ref())
            } else {
                None
            },
            l1_note: if fresh { snap.l1_note.clone() } else { None },
            history,
            deposits: if fresh {
                snap.deposits.clone()
            } else {
                Vec::new()
            },
            withdrawals: record
                .withdrawals
                .iter()
                .filter(|w| w.coin == coin_name(coin))
                .cloned()
                .collect(),
            in_flight: record
                .outgoing
                .iter()
                .filter(|o| parse_chain(&o.chain).is_ok_and(|c| c.coin() == coin))
                .cloned()
                .collect(),
            address_book: settings.address_book.clone(),
            bridge: self.bridge_view(&snap, coin),
            connection_error: snap.connection_error.clone(),
            vm_incoming: if fresh {
                incoming(&snap.deposits, snap.info.as_ref(), coin)
            } else {
                None
            },
            internal_error: journal::last_panic(),
            check_balances: settings.check_balances,
            updates_on: !settings.updates_off,
            updates_possible: crate::update::configured(),
            l1_disagrees: if fresh {
                snap.second_opinion.map(format_btc)
            } else {
                None
            },
            fiat_choice: fiat_choice.into(),
            fiat,
            hardware: record.hardware,
            software_key_warning: record.hardware == Some(false) && !record.software_key_seen,
            backup_check: self
                .backup_check
                .guard()
                .as_ref()
                .filter(|c| Some(&c.wallet) == active.as_ref())
                .map(|c| BackupCheckView {
                    kind: if c.words { "words" } else { "chars" },
                    items: c.items.iter().map(|&(a, b)| [a, b]).collect(),
                }),
            updated: snap.updated,
        }
    }

    /// `coin`'s price in `currency`, once known.
    fn price(&self, coin: Coin, currency: &str) -> Option<f64> {
        self.side(coin)
            .prices
            .guard()
            .as_ref()
            .and_then(|(p, _)| match currency {
                "EUR" => Some(p.eur),
                "USD" => Some(p.usd),
                _ => None,
            })
    }

    /// `coin` at a glance, from its side's snapshot `snap`.
    fn summary(
        &self,
        coin: Coin,
        snap: &Snapshot,
        active: Option<&String>,
        currency: &str,
    ) -> CoinSummary {
        let fresh = snap.wallet.as_ref() == active;
        CoinSummary {
            coin: coin_key(coin),
            l1: if fresh {
                balance(snap.l1.as_ref())
            } else {
                None
            },
            vm: if fresh {
                balance(snap.vm.as_ref())
            } else {
                None
            },
            price: self.price(coin, currency),
            attention: snap.connection_error.is_some()
                || snap.bridge_error.is_some()
                || snap
                    .bridge
                    .as_ref()
                    .is_some_and(|b| b.signer_change.is_some()),
        }
    }

    fn bridge_view(&self, snap: &Snapshot, coin: Coin) -> BridgeView {
        let server = self.bridge(coin).base().to_string();
        let Some(info) = &snap.info else {
            return BridgeView {
                coin: coin_key(coin),
                server,
                ..BridgeView::default()
            };
        };
        let status = snap.status.clone().unwrap_or_default();
        let signer_change = snap.bridge.as_ref().and_then(|b| {
            b.signer_change.as_ref().map(|c| SignerChangeView {
                coin: coin_key(coin),
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
        let amount = |units: u64| coin.format_amount(units);
        BridgeView {
            coin: coin_key(coin),
            server,
            connected: true,
            trusted: snap.bridge.is_some(),
            error: snap.bridge_error.clone(),
            signer_change,
            peg_address: snap.bridge.as_ref().map(|b| b.peg.address(&b.net)),
            min_deposit: snap.bridge.as_ref().map(|b| amount(b.min_deposit)),
            max_deposit: snap
                .bridge
                .as_ref()
                .filter(|b| b.max_deposit > 0)
                .map(|b| amount(b.max_deposit)),
            min_peg_out: snap.bridge.as_ref().map(|b| amount(b.min_peg_out)),
            fee_rate: snap.bridge.as_ref().map(|b| b.fee_rate),
            vm_fee: info.vm_fee.clone(),
            payout_fee: info.payout_fee.clone(),
            paused: status.paused.is_some(),
            solvent: status.audit.as_ref().map(|a| a.solvent),
            locked: status.audit.as_ref().map(|a| a.locked.clone()),
            circulating: status.audit.as_ref().map(|a| a.circulating.clone()),
            l1_syncing: status.bitcoin_sync.is_some_and(|s| s.syncing),
            // Following a rotation is BTC's.
            rotation: if coin == Coin::Btc {
                self.rotation_view()
            } else {
                None
            },
        }
    }

    /// The last rotation followed, unless the user has seen it.
    fn rotation_view(&self) -> Option<RotationView> {
        let settings = self.settings.guard();
        let last = settings.rotations.last()?;
        let to_peg = last.to.peg().ok()?.address(&MAINNET);
        if settings.rotation_seen.as_deref() == Some(to_peg.as_str()) {
            return None;
        }
        Some(RotationView {
            from_peg: last.from.peg().ok()?.address(&MAINNET),
            to_peg,
            chain: last.chain.clone(),
            txid: rotation_txid(last),
        })
    }

    // --- refreshing ------------------------------------------------------

    /// Refreshes everything from the bridge. Calls that arrive while one is
    /// running fold into a single follow-up pass.
    pub fn refresh(&self) {
        let _running = match self.refreshing.try_lock() {
            Ok(running) => running,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => {
                self.refresh_again.store(true, Ordering::SeqCst);
                return;
            }
        };
        loop {
            self.refresh_again.store(false, Ordering::SeqCst);
            self.refresh_once();
            if !self.refresh_again.swap(false, Ordering::SeqCst) {
                break;
            }
        }
    }

    /// A wallet's address on `coin`'s two chains. On the coin's own chain the
    /// bridge answers 404 until the address is registered, which happens here
    /// the first time.
    fn fetch(
        &self,
        api: &dyn Api,
        info: &Facts,
        address: &str,
        coin: Coin,
    ) -> (Option<AddressView>, Option<AddressView>, Option<String>) {
        let vm = api.address(coin.vm(), address).ok();
        if !info.l1_wallet {
            return (None, vm, Some("unavailable".into()));
        }
        match api.address(coin.l1(), address) {
            Ok(v) => (Some(v), vm, None),
            Err(e) if e.status == 404 => {
                let _ = api.watch(coin.l1(), address);
                let v = api.address(coin.l1(), address).ok();
                let note = v.is_none().then(|| "watching".to_string());
                (v, vm, note)
            }
            Err(e) if e.status == 503 => (None, vm, Some("syncing".into())),
            Err(e) => (None, vm, Some(e.message)),
        }
    }

    fn refresh_once(&self) {
        for coin in Coin::ALL {
            self.refresh_side(coin);
            self.note_new_payments(coin);
            self.refresh_prices(coin, false);
        }
        self.ask_attestations();
    }

    /// Refreshes `coin`'s side from its bridge.
    fn refresh_side(&self, coin: Coin) {
        enum Info {
            Btc(Box<BridgeInfo>),
            Doge(Box<DogeBridgeInfo>),
        }
        let side = self.side(coin);
        let api = self.bridge(coin);
        let info = match coin {
            Coin::Btc => api.info().map(|i| Info::Btc(Box::new(i))),
            Coin::Doge => api.doge_info().map(|i| Info::Doge(Box::new(i))),
        };
        let info = match info {
            Ok(info) => info,
            Err(e) => {
                let mut snap = side.snapshot.guard();
                if snap.connection_error.is_none() {
                    journal::warn(&format!(
                        "can't reach {}'s bridge: {}",
                        coin.vm().name(),
                        e.message
                    ));
                }
                snap.connection_error = Some(e.message);
                return;
            }
        };
        let verify = |info: &Info| {
            let checked = match info {
                Info::Btc(i) => peg::verify(i, &self.pinned(coin)),
                Info::Doge(i) => peg::verify_doge(i, &self.pinned(coin)),
            };
            match checked {
                Ok(b) => (Some(b), None),
                Err(e) => (None, Some(Failure::from(e))),
            }
        };
        let (mut bridge, mut bridge_error) = verify(&info);
        // A signer change on BTCVM's bridge: look for the old signers' own
        // move to the new set, every ten minutes, and follow it if it checks
        // out. DogecoinVM's bridge can't hand over to a new set yet.
        let change = bridge.as_ref().and_then(|b| b.signer_change.clone());
        if let Some(change) = change.filter(|_| coin == Coin::Btc) {
            let due = now().saturating_sub(*side.rotation_checked.guard()) >= 600;
            if due && self.follow_rotation(&*api, &change) {
                (bridge, bridge_error) = verify(&info);
            }
        }
        let facts = match &info {
            Info::Btc(i) => Facts::from(&**i),
            Info::Doge(i) => Facts::from(&**i),
        };
        let status = api.status().ok();
        let active = self.active_id();
        let mut snap = Snapshot {
            wallet: active.clone(),
            ..Snapshot::default()
        };
        let ids: Vec<String> = self
            .settings
            .lock()
            .unwrap()
            .wallets
            .iter()
            .map(|w| w.id.clone())
            .collect();
        for id in ids {
            let Some(address) = self.address_for(&id, coin) else {
                continue;
            };
            let (l1, vm, note) = self.fetch(&*api, &facts, &address, coin);
            snap.balances
                .insert(id.clone(), (balance(l1.as_ref()), balance(vm.as_ref())));
            if Some(&id) == active.as_ref() {
                snap.deposits = api.deposits(&address).unwrap_or_default();
                self.follow_withdrawals(&*api, &id, coin);
                self.settle(&id, coin.vm(), vm.as_ref());
                self.settle(&id, coin.l1(), l1.as_ref());
                if coin == Coin::Btc {
                    snap.second_opinion = self.second_opinion(&*api, &address, l1.as_ref());
                }
                snap.vm = vm;
                snap.l1 = l1;
                snap.l1_note = note;
            }
        }
        snap.info = Some(facts);
        snap.bridge = bridge;
        {
            let before = side.snapshot.guard();
            let name = coin.vm().name();
            if before.connection_error.is_some() {
                journal::info(&format!("{name}'s bridge answers again"));
            }
            match (&before.bridge_error, &bridge_error) {
                (None, Some(e)) => {
                    journal::error(&format!("{name}'s bridge failed the checks: {}", e.message))
                }
                (Some(_), None) => {
                    journal::info(&format!("{name}'s bridge passes the checks again"))
                }
                _ => {}
            }
        }
        snap.bridge_error = bridge_error;
        snap.status = status;
        snap.updated = now();
        // A wallet switched while this ran keeps its fresh state.
        if self.active_id() == active {
            *side.snapshot.guard() = snap;
        }
    }

    /// Notes payments to the active wallet on `coin`'s chains not seen
    /// before, for the window to announce. A first look at a wallet, or at
    /// one of its chains, only remembers what is there.
    fn note_new_payments(&self, coin: Coin) {
        let (wallet, entries) = {
            let snap = self.side(coin).snapshot.guard();
            let Some(wallet) = snap.wallet.clone() else {
                return;
            };
            let mut entries = Vec::new();
            for (chain, view) in [
                (chain_name(coin.l1()), &snap.l1),
                (chain_name(coin.vm()), &snap.vm),
            ] {
                if let Some(view) = view {
                    for h in &view.history {
                        entries.push((chain, h.txid.clone(), h.net.clone(), h.confirmations));
                    }
                }
            }
            (wallet, entries)
        };
        let mut seen = self.seen.guard();
        if seen.wallet.as_deref() != Some(wallet.as_str()) {
            *seen = Seen {
                wallet: Some(wallet),
                ..Seen::default()
            };
        }
        let known: HashSet<&'static str> = seen.chains.clone();
        let mut notices = Vec::new();
        for (chain, txid, net, confirmations) in entries {
            seen.chains.insert(chain);
            let new = seen.txids.insert(format!("{chain}:{txid}"));
            if new && known.contains(chain) {
                // Only what comes in: a net loss doesn't parse as an amount.
                if let Ok(amount) = coin.parse_amount(&net)
                    && amount > 0
                {
                    notices.push(Notice {
                        chain,
                        amount: coin.format_amount(amount),
                        confirmed: confirmations > 0,
                    });
                }
            }
        }
        drop(seen);
        if !notices.is_empty() {
            for n in &notices {
                journal::info(&format!(
                    "received {} {} on {}",
                    n.amount,
                    coin.ticker(),
                    n.chain
                ));
            }
            self.notices.guard().extend(notices);
        }
    }

    /// The payments received since the last call.
    pub fn take_notices(&self) -> Vec<Notice> {
        std::mem::take(&mut *self.notices.guard())
    }

    /// Fetches `coin`'s price every ten minutes, or now, unless the user
    /// turned values off: BTC's from mempool.space, DOGE's from CoinGecko.
    fn refresh_prices(&self, coin: Coin, now_please: bool) {
        let choice = fiat_choice(&self.settings.guard(), &self.language());
        if choice == "none" {
            return;
        }
        let side = self.side(coin);
        let fresh = side
            .prices
            .guard()
            .as_ref()
            .is_some_and(|(_, at)| now().saturating_sub(*at) < 600);
        if fresh && !now_please {
            return;
        }
        let api = self.bridge(coin);
        let fetched = match coin {
            Coin::Btc => api.prices(),
            Coin::Doge => api.doge_prices(),
        };
        if let Ok(p) = fetched
            && p.usd.is_finite()
            && p.eur.is_finite()
            && p.usd > 0.0
            && p.eur > 0.0
        {
            *side.prices.guard() = Some((p, now()));
        }
    }

    /// Asks Windows, once per wallet and run, whether each wallet's Windows
    /// Hello key is certified to be in a TPM. Asks the user nothing.
    fn ask_attestations(&self) {
        let ids: Vec<String> = self
            .settings
            .guard()
            .wallets
            .iter()
            .filter(|w| w.hardware.is_none())
            .map(|w| w.id.clone())
            .collect();
        for id in ids {
            if !self.attestation_asked.guard().insert(id.clone()) {
                continue;
            }
            match self.vault(&id).attested() {
                Ok(Some(hardware)) => {
                    if !hardware {
                        journal::warn(&format!(
                            "Windows can't certify wallet {id}'s Windows Hello key is in a TPM"
                        ));
                    }
                    self.update_record(&id, |r| r.hardware = Some(hardware));
                }
                Ok(None) => {}
                Err(e) => journal::warn(&format!("asking about wallet {id}'s key: {e}")),
            }
        }
    }

    /// mempool.space's confirmed Bitcoin balance for `address`, when the user
    /// asked for a second opinion and it differs from the bridge's `view`.
    fn second_opinion(
        &self,
        api: &dyn Api,
        address: &str,
        view: Option<&AddressView>,
    ) -> Option<u64> {
        if !self.settings.guard().check_balances {
            return None;
        }
        let bridge: u64 = view?
            .utxos
            .iter()
            .filter(|u| u.confirmations > 0)
            .filter_map(|u| u.value.parse::<u64>().ok())
            .sum();
        let other = api.bitcoin_balance(address).ok()?;
        if other != bridge {
            journal::warn(&format!(
                "the bridge says {} BTC on Bitcoin and mempool.space {} BTC",
                format_btc(bridge),
                format_btc(other)
            ));
        }
        (other != bridge).then_some(other)
    }

    pub fn updates_on(&self) -> bool {
        !self.settings.guard().updates_off
    }

    pub fn set_updates(&self, on: bool) -> View {
        self.update(|s| s.updates_off = !on);
        self.view()
    }

    pub fn set_check_balances(&self, on: bool) -> View {
        self.update(|s| s.check_balances = on);
        self.refresh();
        self.view()
    }

    /// Checks each withdrawal of wallet `id` from `coin`'s VM not yet
    /// confirmed on the coin's own chain.
    fn follow_withdrawals(&self, api: &dyn Api, id: &str, coin: Coin) {
        let open: Vec<String> = self
            .record(id)
            .map(|r| r.withdrawals)
            .unwrap_or_default()
            .into_iter()
            .filter(|w| w.coin == coin_name(coin))
            .filter(|w| w.status != "paid" || w.payment_confirmations.unwrap_or(0) == 0)
            .map(|w| w.txid)
            .collect();
        for txid in open {
            let Ok(s) = api.peg_out(&txid) else { continue };
            self.update_record(id, |r| {
                if let Some(w) = r.withdrawals.iter_mut().find(|w| w.txid == txid) {
                    w.status = if s.status == "unknown" && w.status == "sending" {
                        "sending".into()
                    } else {
                        s.status
                    };
                    w.pays = s.pays;
                    w.payment_txid = s.payment_txid;
                    w.payment_confirmations = s.payment_confirmations;
                }
            });
        }
    }

    /// Forgets payments the chain shows confirmed, or hasn't shown after an
    /// hour (dropped by the network).
    fn settle(&self, id: &str, chain: Chain, view: Option<&AddressView>) {
        let Some(view) = view else { return };
        let seen: HashMap<&str, i64> = view
            .history
            .iter()
            .map(|h| (h.txid.as_str(), h.confirmations))
            .collect();
        let name = chain_name(chain);
        let time = now();
        let keep = |o: &Outgoing| {
            if o.chain != name {
                return true;
            }
            match seen.get(o.txid.as_str()) {
                Some(&confirmations) => confirmations <= 0,
                None => time.saturating_sub(o.time) < 3600,
            }
        };
        let changed = self
            .record(id)
            .is_some_and(|r| r.outgoing.iter().any(|o| !keep(o)));
        if changed {
            self.update_record(id, |r| r.outgoing.retain(|o| keep(o)));
        }
    }

    /// The signer set the wallet trusts for `coin`, pinned as the built-in
    /// one is.
    fn pinned(&self, coin: Coin) -> Pinned {
        let side = self.side(coin);
        Pinned {
            signers: side.trusted.guard().clone(),
            ..side.built_in.clone()
        }
    }

    /// Looks for the move by which the trusted signers handed the peg to the
    /// set the bridge reports, checks it, and follows it: on BTCVM, where
    /// the reserve moves first, then on Bitcoin. Up to three rotations are
    /// followed, for a wallet that missed some. True when it followed.
    fn follow_rotation(&self, api: &dyn Api, change: &SignerChange) -> bool {
        let side = self.side(Coin::Btc);
        *side.rotation_checked.guard() = now();
        let mut from = change.trusted.clone();
        let mut proofs = Vec::new();
        for _ in 0..3 {
            let Some(proof) = self.find_move(api, &from, &change.reported) else {
                journal::warn(&format!(
                    "no checked move yet from the signers of {} to those of {}",
                    change.trusted_peg.address(&MAINNET),
                    change.reported_peg.address(&MAINNET)
                ));
                return false;
            };
            from = proof.to.clone();
            proofs.push(proof);
            if from == change.reported {
                break;
            }
        }
        if from != change.reported {
            return false;
        }
        for p in &proofs {
            journal::info(&format!(
                "followed a rotation of the peg's signers: {} on {}",
                rotation_txid(p),
                p.chain
            ));
        }
        self.update(|s| s.rotations.extend(proofs));
        *side.trusted.guard() = from;
        true
    }

    /// A checked move from `from`: to `target` if one is found, or else to
    /// the next set in between, whose keys a later spend reveals.
    fn find_move(&self, api: &dyn Api, from: &Signers, target: &Signers) -> Option<RotationProof> {
        let peg = from.peg().ok()?.address(&MAINNET);
        let target_hash = sha256(&target.witness_script().ok()?);
        for (chain, txid) in self.move_candidates(api, &peg) {
            let Ok(hex) = api.raw_tx(chain, &txid) else {
                continue;
            };
            let Ok(raw) = hex::decode(hex.trim()) else {
                continue;
            };
            let Some(tagged) = rotation::migrate_target(&raw) else {
                continue;
            };
            let to = if tagged == target_hash {
                target.clone()
            } else {
                match self.revealed_set(api, chain, &tagged) {
                    Some(set) => set,
                    None => continue,
                }
            };
            if let Some(proof) = self.check_move(api, chain, from, &to, &raw) {
                return Some(proof);
            }
        }
        None
    }

    /// Transactions that may be moves from the peg `address`: its latest
    /// spends on BTCVM, then its latest transactions on Bitcoin with a BVMM
    /// tag.
    fn move_candidates(&self, api: &dyn Api, address: &str) -> Vec<(Chain, String)> {
        let mut out = Vec::new();
        if let Ok(view) = api.address(Chain::Btcvm, address) {
            out.extend(
                view.history
                    .iter()
                    .filter(|h| h.net.starts_with('-'))
                    .take(40)
                    .map(|h| (Chain::Btcvm, h.txid.clone())),
            );
        }
        let tag = format!("6a24{}", hex::encode(rotation::MIGRATE_TAG));
        if let Ok(txs) = api.bitcoin_address_txs(address) {
            out.extend(
                txs.iter()
                    .filter(|t| t.vout.iter().any(|o| o.scriptpubkey.starts_with(&tag)))
                    .map(|t| (Chain::Bitcoin, t.txid.clone())),
            );
        }
        out
    }

    /// The set whose peg has the P2WSH `program`, from a spend of it, whose
    /// witness reveals its script.
    fn revealed_set(&self, api: &dyn Api, chain: Chain, program: &[u8; 32]) -> Option<Signers> {
        let address = Destination::new(AddressKind::P2wsh, program)
            .ok()?
            .address(&MAINNET);
        let txids: Vec<String> = match chain {
            Chain::Btcvm => api
                .address(Chain::Btcvm, &address)
                .ok()?
                .history
                .iter()
                .filter(|h| h.net.starts_with('-'))
                .take(20)
                .map(|h| h.txid.clone())
                .collect(),
            Chain::Bitcoin => api
                .bitcoin_address_txs(&address)
                .ok()?
                .into_iter()
                .take(20)
                .map(|t| t.txid)
                .collect(),
            // Following a rotation is BTCVM's: DogecoinVM's bridge can't
            // move to a new set yet.
            Chain::Dogecoin | Chain::Dogecoinvm => return None,
        };
        txids.iter().find_map(|txid| {
            let raw = hex::decode(api.raw_tx(chain, txid).ok()?.trim()).ok()?;
            rotation::signers_spent(&raw, program)
        })
    }

    /// Checks `raw` as a move from `from` to `to`, with the transaction that
    /// made each of its first few coins in turn.
    fn check_move(
        &self,
        api: &dyn Api,
        chain: Chain,
        from: &Signers,
        to: &Signers,
        raw: &[u8],
    ) -> Option<RotationProof> {
        let parsed = parse_tx(raw).ok()?;
        for (prev_txid, _) in parsed.inputs.iter().take(3) {
            let Ok(prev) = api.raw_tx(chain, prev_txid) else {
                continue;
            };
            let prev = HashMap::from([(prev_txid.clone(), prev.trim().to_string())]);
            if rotation::verify_move(from, to, raw, &prev).is_ok() {
                return Some(RotationProof {
                    chain: chain_name(chain).into(),
                    from: from.clone(),
                    to: to.clone(),
                    tx: hex::encode(raw),
                    prev: prev.into_iter().collect(),
                    verified: now(),
                });
            }
        }
        None
    }

    /// Checks again now for the move behind a signer change, at the user's
    /// request.
    pub fn check_rotation_now(&self) -> View {
        *self.side(Coin::Btc).rotation_checked.guard() = 0;
        self.refresh();
        self.view()
    }

    /// The user has seen the rotation the wallet followed.
    pub fn rotation_seen(&self) -> View {
        let last = self
            .settings
            .guard()
            .rotations
            .last()
            .and_then(|p| p.to.peg().ok())
            .map(|d| d.address(&MAINNET));
        self.update(|s| s.rotation_seen = last);
        self.view()
    }

    /// A signer change to warn about once, on either coin's bridge, if one
    /// is new.
    pub fn new_signer_change(&self) -> Option<SignerChangeView> {
        for coin in Coin::ALL {
            let change = {
                let snap = self.side(coin).snapshot.guard();
                self.bridge_view(&snap, coin).signer_change
            };
            let Some(change) = change else { continue };
            let already = {
                let s = self.settings.guard();
                match coin {
                    Coin::Btc => s.notified_signer_change.clone(),
                    Coin::Doge => s.notified_doge_signer_change.clone(),
                }
            };
            if already.as_deref() == Some(change.reported_peg.as_str()) {
                continue;
            }
            let peg = Some(change.reported_peg.clone());
            self.update(|s| match coin {
                Coin::Btc => s.notified_signer_change = peg,
                Coin::Doge => s.notified_doge_signer_change = peg,
            });
            return Some(change);
        }
        None
    }

    // --- wallets -------------------------------------------------------------

    /// Adds a wallet holding `secret` and makes it the active one. It shares
    /// the other wallets' Windows Hello key, if there is one, so Windows
    /// Hello asks once instead of twice; each vault still opens only with
    /// its own signature.
    fn adopt(&self, secret: &Secret, name: &str, made_here: bool) -> Outcome<String> {
        let name = wallet_name(name)?;
        let address = secret.key()?.destination().address(&MAINNET);
        let ids: Vec<WalletRecord> = self.settings.guard().wallets.clone();
        if let Some(w) = ids
            .iter()
            .find(|w| self.address_of(&w.id).as_deref() == Some(&address))
        {
            return fail(format!(
                "that key is already in this app, as wallet \"{}\"",
                self.display_name(w)
            ));
        }
        let id = if ids.is_empty() && !self.vault(FIRST).has_key() {
            FIRST.to_string()
        } else {
            let mut raw = [0u8; 6];
            rand_id(&mut raw);
            hex::encode(raw)
        };
        let vault = self.vault(&id);
        match ids.iter().find_map(|w| self.vault(&w.id).gate_key().ok()) {
            Some(shared) => match vault.store_with(secret, &shared) {
                // Windows Hello lost it: a new one, then.
                Err(VaultError::GateMissing) => vault.store(secret)?,
                stored => stored?,
            },
            None => vault.store(secret)?,
        }
        self.update(|s| {
            s.wallets.push(WalletRecord {
                id: id.clone(),
                name,
                created: now(),
                backup_confirmed: !made_here,
                backup_required: made_here,
                hardware: None,
                software_key_seen: false,
                doge_address: phrase_doge_address(secret),
                ..WalletRecord::default()
            });
            s.active = Some(id.clone());
        });
        self.forget_views();
        Ok(id)
    }

    /// Makes a new wallet, with a twelve-word recovery phrase. `show_backup`
    /// shows it outside the web view and says whether the user saved it;
    /// then they type some words back. Until then nothing can be received,
    /// as this PC holds the only copy.
    pub fn create(&self, name: &str, show_backup: impl Fn(&BackupText) -> bool) -> Outcome<View> {
        let secret = Secret::new_phrase();
        let id = self.adopt(&secret, name, true)?;
        let text = BackupText::of(&secret)?;
        drop(secret);
        if show_backup(&text) {
            self.start_backup_check(&id, &text);
        }
        drop(text);
        self.refresh();
        Ok(self.view())
    }

    /// Adds a wallet from a backup the user already has, from the
    /// clipboard: a recovery phrase, or a key.
    pub fn import(&self, name: &str, text: Zeroizing<String>) -> Outcome<View> {
        let secret = Secret::parse(&text)?;
        drop(text);
        self.adopt(&secret, name, false)?;
        self.refresh();
        Ok(self.view())
    }

    pub fn select(&self, id: &str) -> Outcome<View> {
        if self.record(id).is_none() {
            return fail("there is no such wallet");
        }
        self.update(|s| s.active = Some(id.to_string()));
        *self.backup_check.guard() = None;
        self.forget_views();
        self.refresh();
        Ok(self.view())
    }

    pub fn rename(&self, id: &str, name: &str) -> Outcome<View> {
        let name = wallet_name(name)?;
        if self.record(id).is_none() {
            return fail("there is no such wallet");
        }
        self.update_record(id, |r| r.name = name);
        Ok(self.view())
    }

    /// Shows the active wallet's backup after Windows Hello; when the user
    /// says they saved it, they type some of it back.
    pub fn backup(&self, show_backup: impl Fn(&BackupText) -> bool) -> Outcome<View> {
        let id = self
            .active_id()
            .map_or_else(|| fail("there is no wallet on this PC"), Ok)?;
        let secret = self.vault(&id).unlock_secret()?;
        self.note_doge_address(&id, &secret);
        let text = BackupText::of(&secret)?;
        drop(secret);
        if show_backup(&text) {
            self.start_backup_check(&id, &text);
        }
        Ok(self.view())
    }

    /// Keeps a phrase wallet's DOGE address once its phrase is unlocked.
    fn note_doge_address(&self, id: &str, secret: &Secret) {
        if let Some(address) = phrase_doge_address(secret)
            && self
                .record(id)
                .is_some_and(|r| r.doge_address.as_deref() != Some(address.as_str()))
        {
            self.update_record(id, |r| r.doge_address = Some(address));
        }
    }

    /// Learns the active wallet's DOGE address from its recovery phrase,
    /// after Windows Hello: a phrase makes another key for DOGE than for BTC.
    pub fn learn_doge_address(&self) -> Outcome<View> {
        let id = self
            .active_id()
            .map_or_else(|| fail("there is no wallet on this PC"), Ok)?;
        let secret = self.vault(&id).unlock_secret()?;
        self.note_doge_address(&id, &secret);
        drop(secret);
        self.refresh();
        Ok(self.view())
    }

    /// Shows `coin`: "btc" or "doge".
    pub fn set_coin(&self, coin: &str) -> Outcome<View> {
        let coin = parse_coin(coin)?;
        *self.pending.guard() = None;
        self.update(|s| s.coin = (coin != Coin::Btc).then(|| coin_key(coin).to_string()));
        Ok(self.view())
    }

    /// Picks what to ask back: three of the words, or two groups of four of
    /// the key's characters.
    fn start_backup_check(&self, wallet: &str, text: &BackupText) {
        let check = match &text.words {
            Some(words) => {
                let list: Vec<&str> = words.split(' ').collect();
                let picked = pick(list.len(), 3);
                BackupCheck {
                    wallet: wallet.into(),
                    words: true,
                    items: picked.iter().map(|&i| (i + 1, i + 1)).collect(),
                    expected: Zeroizing::new(picked.iter().map(|&i| list[i].to_string()).collect()),
                }
            }
            None => {
                let picked = pick(text.wif.len() / 4, 2);
                BackupCheck {
                    wallet: wallet.into(),
                    words: false,
                    items: picked.iter().map(|&k| (4 * k + 1, 4 * k + 4)).collect(),
                    expected: Zeroizing::new(
                        picked
                            .iter()
                            .map(|&k| text.wif[4 * k..4 * k + 4].to_string())
                            .collect(),
                    ),
                }
            }
        };
        *self.backup_check.guard() = Some(check);
    }

    /// Checks what the user typed back from their backup. Right, and the
    /// wallet counts as backed up. A word may be its first four letters.
    pub fn verify_backup(&self, answers: &[String]) -> Outcome<View> {
        let wallet = {
            let check = self.backup_check.guard();
            let Some(c) = check.as_ref() else {
                return fail("there is no backup to check");
            };
            let right = answers.len() == c.expected.len()
                && c.expected.iter().zip(answers).all(|(want, got)| {
                    let got = got.trim();
                    if c.words {
                        let got = got.to_ascii_lowercase();
                        *want == got || (got.len() >= 4 && want.starts_with(got.as_str()))
                    } else {
                        want == got
                    }
                });
            if !right {
                return fail(
                    "that doesn't match the backup you were shown: look at it again, or show it again",
                );
            }
            c.wallet.clone()
        };
        *self.backup_check.guard() = None;
        self.mark_backed_up(&wallet);
        journal::info(&format!("wallet {wallet}'s backup was checked"));
        Ok(self.view())
    }

    pub fn cancel_backup_check(&self) -> View {
        *self.backup_check.guard() = None;
        self.view()
    }

    /// Puts the active wallet back from its backup when its vault can't be
    /// opened: the key must be this wallet's.
    pub fn restore(&self, text: Zeroizing<String>) -> Outcome<View> {
        let id = self
            .active_id()
            .map_or_else(|| fail("there is no wallet on this PC"), Ok)?;
        let secret = Secret::parse(&text)?;
        drop(text);
        self.vault(&id).restore(&secret)?;
        self.note_doge_address(&id, &secret);
        self.mark_backed_up(&id);
        self.refresh();
        Ok(self.view())
    }

    /// Removes the active wallet from this PC after Windows Hello. `confirm`
    /// asks the user, outside the web view.
    pub fn remove(&self, confirm: impl Fn() -> bool) -> Outcome<View> {
        let id = self
            .active_id()
            .map_or_else(|| fail("there is no wallet on this PC"), Ok)?;
        if !confirm() {
            return Ok(self.view());
        }
        let vault = self.vault(&id);
        // Its Windows Hello key stays if another wallet shares it.
        let others: Vec<String> = self
            .settings
            .guard()
            .wallets
            .iter()
            .filter(|w| w.id != id)
            .map(|w| w.id.clone())
            .collect();
        let shared = vault.gate_key().ok().is_some_and(|g| {
            others
                .iter()
                .any(|o| self.vault(o).gate_key().ok().as_ref() == Some(&g))
        });
        match vault.remove(shared) {
            Ok(()) => {}
            // A vault that can't be opened is set aside instead: the key may
            // still be recoverable from it some other way.
            Err(e) if e.needs_restore() => {
                if vault.has_key() {
                    vault.set_aside()?;
                }
            }
            Err(e) => return Err(e.into()),
        }
        self.update(|s| {
            s.wallets.retain(|w| w.id != id);
            s.active = s.wallets.first().map(|w| w.id.clone());
        });
        *self.backup_check.guard() = None;
        if id != FIRST {
            // Its folder goes too, when nothing was set aside in it.
            let _ = std::fs::remove_dir(store::vault_dir(&self.dir, &id));
        }
        self.forget_views();
        self.refresh();
        Ok(self.view())
    }

    fn mark_backed_up(&self, id: &str) {
        self.update_record(id, |r| {
            r.backup_confirmed = true;
            r.backup_required = false;
        });
    }

    /// Drops what was shown for the previous wallet and any payment waiting.
    fn forget_views(&self) {
        *self.pending.guard() = None;
        for coin in Coin::ALL {
            let mut snap = self.side(coin).snapshot.guard();
            snap.wallet = None;
            snap.vm = None;
            snap.l1 = None;
            snap.l1_note = None;
            snap.deposits.clear();
        }
    }

    // --- the address book --------------------------------------------------------

    pub fn add_contact(&self, name: &str, address: &str, chain: &str) -> Outcome<View> {
        let chain = parse_chain(chain)?;
        let net = chain.coin().network();
        let canonical = book::canonical(address, &net)?;
        let dest = decode_address(&canonical, &net)?;
        if self
            .side(chain.coin())
            .snapshot
            .guard()
            .bridge
            .as_ref()
            .is_some_and(|b| b.is_peg(&dest))
        {
            return fail("that is the bridge's own address; it can't be paid directly");
        }
        let current = self.settings.guard().address_book.clone();
        let next = book::add(&current, name, &canonical, chain)?;
        self.update(|s| s.address_book = next);
        Ok(self.view())
    }

    pub fn rename_contact(&self, address: &str, chain: &str, name: &str) -> Outcome<View> {
        let chain = parse_chain(chain)?;
        let current = self.settings.guard().address_book.clone();
        let next = book::rename(&current, address, chain, name)?;
        self.update(|s| s.address_book = next);
        Ok(self.view())
    }

    pub fn remove_contact(&self, address: &str, chain: &str) -> Outcome<View> {
        let chain = parse_chain(chain)?;
        self.update(|s| s.address_book = book::remove(&s.address_book, address, chain));
        Ok(self.view())
    }

    /// What the wallet knows of `address` paid on `chain`: one of its own
    /// wallets, a contact (saved for this chain or the other), or nothing;
    /// and whether it looks like one of those without being it.
    fn recognize(
        &self,
        address: &str,
        chain: Chain,
    ) -> (&'static str, Option<String>, Option<String>) {
        let settings = self.settings.guard().clone();
        let own: Vec<(String, String)> = settings
            .wallets
            .iter()
            .filter_map(|w| {
                self.address_for(&w.id, chain.coin())
                    .map(|a| (a, self.display_name(w)))
            })
            .collect();
        let lookalike = own
            .iter()
            .map(|(a, n)| (a.as_str(), n.clone()))
            .chain(
                settings
                    .address_book
                    .iter()
                    .map(|c| (c.address.as_str(), c.name.clone())),
            )
            .find(|(a, _)| book::lookalike(address, a))
            .map(|(_, n)| n);
        if let Some((_, name)) = own.iter().find(|(a, _)| a == address) {
            return ("own", Some(name.clone()), lookalike);
        }
        match book::find(&settings.address_book, address, chain) {
            Some(c) if c.chain == chain => ("book", Some(c.name.clone()), lookalike),
            Some(c) => ("bookOtherChain", Some(c.name.clone()), lookalike),
            None => ("new", None, lookalike),
        }
    }

    // --- fees -------------------------------------------------------------------

    /// The Bitcoin fee rates to offer. mempool.space's are kept only within
    /// the wallet's bounds.
    pub fn fee_options(&self) -> FeeOptions {
        let bridge = self
            .side(Coin::Btc)
            .snapshot
            .guard()
            .bridge
            .as_ref()
            .map(|b| b.fee_rate);
        let fees: Option<FeeEstimates> = self.bridge(Coin::Btc).recommended_fees().ok();
        let rate = |f: Option<f64>| {
            f.filter(|r| r.is_finite())
                .map(|r| r.ceil() as u64)
                .filter(|r| (1..=MAX_FEE_RATE).contains(r))
        };
        FeeOptions {
            bridge,
            fastest: rate(fees.map(|f| f.fastest_fee)),
            half_hour: rate(fees.map(|f| f.half_hour_fee)),
            hour: rate(fees.map(|f| f.hour_fee)),
            economy: rate(fees.map(|f| f.economy_fee)),
            minimum: rate(fees.map(|f| f.minimum_fee)),
            max: MAX_FEE_RATE,
        }
    }

    /// The fee rate for a Bitcoin payment: the user's, or the bridge's
    /// estimate. The core refuses one outside its bounds.
    fn fee_rate(bridge: &VerifiedBridge, chosen: Option<u64>) -> u64 {
        chosen.unwrap_or(bridge.fee_rate)
    }

    // --- receiving, values and the history -----------------------------------

    /// The active wallet's address for the coin shown as a QR code, for a
    /// phone to scan. Just the address, with no "bitcoin:", since it is the
    /// same on the coin's VM. A BTC address goes in capitals: a QR code holds
    /// them more compactly, and wallets read them as the same address. A
    /// DOGE one can't: in base58, case matters.
    pub fn receive_qr(&self) -> Outcome<Qr> {
        let view = self.view();
        let address = view
            .address
            .map_or_else(|| fail("back up your key before receiving"), Ok)?;
        let text = if view.coin == coin_key(Coin::Btc) {
            address.to_ascii_uppercase()
        } else {
            address.clone()
        };
        let code = qrcode::QrCode::with_error_correction_level(text.as_bytes(), qrcode::EcLevel::M)
            .map_err(|e| Failure {
                message: format!("the QR code: {e}"),
                untrusted: false,
                canceled: false,
            })?;
        Ok(Qr {
            width: code.width(),
            modules: code
                .to_colors()
                .iter()
                .map(|c| if *c == qrcode::Color::Dark { '1' } else { '0' })
                .collect(),
            address,
        })
    }

    pub fn set_fiat(&self, currency: &str) -> Outcome<View> {
        if !["EUR", "USD", "none"].contains(&currency) {
            return fail("unknown currency");
        }
        self.update(|s| s.fiat = Some(currency.to_string()));
        for coin in Coin::ALL {
            self.refresh_prices(coin, true);
        }
        Ok(self.view())
    }

    /// The user has seen that Windows can't certify the active wallet's key.
    pub fn software_key_seen(&self) -> View {
        if let Some(id) = self.active_id() {
            self.update_record(&id, |r| r.software_key_seen = true);
        }
        self.view()
    }

    /// The active wallet's history of the coin shown, as far back as the
    /// bridge returns it, as CSV, with a name for the file.
    pub fn history_csv(&self) -> Outcome<(String, String)> {
        let id = self
            .active_id()
            .map_or_else(|| fail("there is no wallet on this PC"), Ok)?;
        let coin = self.active_coin();
        let address = self.address_for(&id, coin).unwrap_or_default();
        let es = self.language() == "es";
        let mut rows = Vec::new();
        {
            let snap = self.side(coin).snapshot.guard();
            if snap.wallet.as_deref() != Some(id.as_str()) {
                return fail("this wallet's balance hasn't loaded yet");
            }
            for (chain, view) in [(coin.l1().name(), &snap.l1), (coin.vm().name(), &snap.vm)] {
                for h in view.iter().flat_map(|v| &v.history) {
                    rows.push((
                        h.time,
                        chain,
                        h.txid.clone(),
                        h.net.clone(),
                        h.confirmations,
                    ));
                }
            }
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r.0.unwrap_or(i64::MAX)));
        let ticker = coin.ticker().to_ascii_lowercase();
        let mut csv = if es {
            format!("fecha_utc,red,txid,cantidad_{ticker},confirmaciones,direccion\r\n")
        } else {
            format!("date_utc,network,txid,amount_{ticker},confirmations,address\r\n")
        };
        for (time, chain, txid, net, confirmations) in rows {
            let date = time
                .and_then(|t| u64::try_from(t).ok())
                .map(journal::timestamp)
                .unwrap_or_default();
            csv.push_str(&format!(
                "{date},{chain},{txid},{net},{confirmations},{address}\r\n"
            ));
        }
        let name: String = self
            .record(&id)
            .map(|r| self.display_name(&r))
            .unwrap_or_default()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let day = journal::timestamp(now())[..10].to_string();
        let vm = coin.vm().name().to_ascii_lowercase();
        Ok((format!("madblocks-{vm}-{name}-{day}.csv"), csv))
    }

    // --- payments: review, then sign exactly what was reviewed ---------------

    fn verified(&self, coin: Coin) -> Outcome<VerifiedBridge> {
        let snap = self.side(coin).snapshot.guard();
        if let Some(e) = &snap.bridge_error {
            return Err(e.clone());
        }
        snap.bridge
            .clone()
            .map_or_else(|| fail("the bridge isn't connected yet"), Ok)
    }

    /// The active wallet and its address for `coin`.
    fn own(&self, coin: Coin) -> Outcome<(String, Destination)> {
        let id = self
            .active_id()
            .map_or_else(|| fail("there is no wallet on this PC"), Ok)?;
        let address = match self.address_for(&id, coin) {
            Some(address) => address,
            None if coin == Coin::Doge && self.address_of(&id).is_some() => {
                return fail(
                    "this wallet's DOGE address comes from its recovery phrase: show it once with Windows Hello first",
                );
            }
            None => return fail("this wallet's vault can't be read"),
        };
        Ok((id, decode_address(&address, &coin.network())?))
    }

    /// The active wallet's coins on `chain` it can spend: confirmed, not
    /// spent by a payment still in flight, with the transactions that made
    /// them.
    fn coins(
        &self,
        id: &str,
        chain: Chain,
    ) -> Outcome<(Vec<btcvm_wallet_core::Utxo>, HashMap<String, String>)> {
        let utxos = {
            let snap = self.side(chain.coin()).snapshot.guard();
            if snap.wallet.as_deref() != Some(id) {
                return fail("this wallet's balance hasn't loaded yet");
            }
            let view = if chain.is_vm() { &snap.vm } else { &snap.l1 };
            let Some(view) = view else {
                return fail(format!("your {} balance hasn't loaded yet", chain.name()));
            };
            view.utxos.clone()
        };
        let in_use: HashSet<String> = self
            .record(id)
            .map(|r| r.outgoing)
            .unwrap_or_default()
            .iter()
            .filter(|o| o.chain == chain_name(chain))
            .flat_map(|o| o.spent.clone())
            .collect();
        let utxos: Vec<_> = utxos
            .into_iter()
            .filter(|u| u.confirmations > 0 && !in_use.contains(&format!("{}:{}", u.txid, u.vout)))
            .collect();
        let api = self.bridge(chain.coin());
        let mut raw = HashMap::new();
        for txid in utxos.iter().map(|u| u.txid.clone()).collect::<HashSet<_>>() {
            let hex = api.raw_tx(chain, &txid)?;
            raw.insert(txid, hex);
        }
        Ok((utxos, raw))
    }

    pub fn prepare_send(
        &self,
        chain: &str,
        to: &str,
        amount: &str,
        fee_rate: Option<u64>,
    ) -> Outcome<Review> {
        let chain = parse_chain(chain)?;
        let coin = chain.coin();
        let bridge = self.verified(coin)?;
        let amount = coin.parse_amount(amount)?;
        let (id, from) = self.own(coin)?;
        let (utxos, raw) = self.coins(&id, chain)?;
        let coins = Coins {
            utxos: &utxos,
            raw_txs: &raw,
        };
        let to = book::canonical(to.trim(), &coin.network())?;
        let mut bridge_for_fee = bridge.clone();
        bridge_for_fee.fee_rate = Self::fee_rate(&bridge, fee_rate);
        let plan = plan_send(&bridge_for_fee, chain, &from, coins, &to, amount)?;
        let rate = Self::fee_rate(&bridge, fee_rate);
        Ok(self.review(Kind::Send, id, plan, to, amount, &bridge, Some(rate), None))
    }

    /// A deposit of the coin shown, from its own chain to its VM.
    pub fn prepare_deposit(&self, amount: &str, fee_rate: Option<u64>) -> Outcome<Review> {
        let coin = self.active_coin();
        let bridge = self.verified(coin)?;
        if bridge.signer_change.is_some() {
            return Err(paused(coin));
        }
        let amount = coin.parse_amount(amount)?;
        let (id, from) = self.own(coin)?;
        // Registers the address with the bridge; the core checks the answer.
        let told = self
            .bridge(coin)
            .deposit_address(&from.address(&coin.network()))?;
        let (utxos, raw) = self.coins(&id, coin.l1())?;
        let coins = Coins {
            utxos: &utxos,
            raw_txs: &raw,
        };
        let mut bridge_for_fee = bridge.clone();
        bridge_for_fee.fee_rate = Self::fee_rate(&bridge, fee_rate);
        let plan = plan_deposit(&bridge_for_fee, &from, coins, amount, &told)?;
        let rate = Self::fee_rate(&bridge, fee_rate);
        Ok(self.review(
            Kind::Deposit,
            id,
            plan,
            told,
            amount,
            &bridge,
            Some(rate),
            None,
        ))
    }

    /// A withdrawal of the coin shown, from its VM to its own chain.
    pub fn prepare_withdrawal(&self, to: &str, amount: &str) -> Outcome<Review> {
        let coin = self.active_coin();
        let bridge = self.verified(coin)?;
        let amount = coin.parse_amount(amount)?;
        let (id, from) = self.own(coin)?;
        let (utxos, raw) = self.coins(&id, coin.vm())?;
        let coins = Coins {
            utxos: &utxos,
            raw_txs: &raw,
        };
        let to = book::canonical(to.trim(), &coin.network())?;
        let plan = plan_withdrawal(&bridge, &from, coins, amount, &to)?;
        Ok(self.review(Kind::Withdraw, id, plan, to, amount, &bridge, None, None))
    }

    /// The most an action can move now, for the amount field: all the
    /// confirmed coins on its chain less the fee, and for a deposit no more
    /// than the bridge accepts. An address not typed yet is sized as the
    /// largest standard output, so the amount fits whatever it turns out to be.
    pub fn max_amount(
        &self,
        action: &str,
        chain: &str,
        to: &str,
        fee_rate: Option<u64>,
    ) -> Outcome<String> {
        let coin = match action {
            "send" => parse_chain(chain)?.coin(),
            _ => self.active_coin(),
        };
        let bridge = self.verified(coin)?;
        let (id, from) = self.own(coin)?;
        // DOGE's fee doesn't depend on the kind of address paid.
        let largest = |kind| match coin {
            Coin::Btc => Destination::new(kind, &[0u8; 32]).expect("a 32-byte program"),
            Coin::Doge => {
                Destination::new(AddressKind::P2sh, &[0u8; 20]).expect("a 20-byte program")
            }
        };
        let typed = |kind| decode_address(to.trim(), &bridge.net).unwrap_or_else(|_| largest(kind));
        let (chain, dest, data, floor, cap) = match action {
            "send" => (parse_chain(chain)?, typed(AddressKind::P2wsh), None, 0, 0),
            "deposit" => {
                if bridge.signer_change.is_some() {
                    return Err(paused(coin));
                }
                (
                    coin.l1(),
                    bridge.signers.deposit_destination_for(coin, &from)?,
                    None,
                    bridge.min_deposit,
                    bridge.max_deposit,
                )
            }
            "withdraw" => {
                if bridge.signer_change.is_some() {
                    return Err(paused(coin));
                }
                let tag = peg_out_data_for(coin, &typed(AddressKind::P2tr));
                (
                    coin.vm(),
                    bridge.peg.clone(),
                    Some(tag),
                    bridge.min_peg_out,
                    0,
                )
            }
            _ => return fail("unknown action"),
        };
        let (utxos, raw) = self.coins(&id, chain)?;
        let max = max_payment(
            &from,
            &utxos,
            &raw,
            &Request {
                chain,
                to: dest,
                amount: 0,
                data,
                fee_rate: Self::fee_rate(&bridge, fee_rate),
            },
        )?;
        if max < floor {
            let ticker = coin.ticker();
            return fail(format!(
                "after the fee there are {} {ticker}, under the minimum of {} {ticker}",
                coin.format_amount(max),
                coin.format_amount(floor)
            ));
        }
        Ok(coin.format_amount(if cap > 0 { max.min(cap) } else { max }))
    }

    #[allow(clippy::too_many_arguments)]
    fn review(
        &self,
        kind: Kind,
        wallet: String,
        plan: Plan,
        to: String,
        amount: u64,
        bridge: &VerifiedBridge,
        fee_rate: Option<u64>,
        replaces: Option<String>,
    ) -> Review {
        let coin = plan.chain.coin();
        let net = coin.network();
        let info = self.side(coin).snapshot.guard().info.clone();
        let outputs = plan
            .describe(&net)
            .into_iter()
            .map(|o| {
                let dest = o
                    .address
                    .as_deref()
                    .and_then(|a| decode_address(a, &net).ok());
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
                    value: coin.format_amount(o.value),
                    address: o.address,
                    role,
                    withdrawal_to: o.withdrawal_to,
                }
            })
            .collect();
        let vm_fee = info
            .as_ref()
            .and_then(|i| i.vm_fee.as_deref())
            .and_then(|f| coin.parse_amount(f).ok());
        let tiers = info.as_ref().map(|i| {
            i.confirmation_tiers
                .iter()
                .find(|t| coin.parse_amount(&t.up_to).is_ok_and(|up| amount <= up))
                .map_or(i.deposit_confirmations, |t| t.confirmations)
        });
        // What the destination is: for a withdrawal, the address on the
        // coin's own chain the bridge pays; for a deposit, the deposit
        // address checked above.
        let (to_known, to_name, lookalike) = match kind {
            Kind::Deposit => ("deposit", None, None),
            Kind::Withdraw => self.recognize(&to, coin.l1()),
            Kind::Send => self.recognize(&to, plan.chain),
        };
        let wallet_name = self
            .record(&wallet)
            .map(|r| self.display_name(&r))
            .unwrap_or_default();
        let review = Review {
            id: 0,
            kind: if replaces.is_some() {
                "bump"
            } else {
                kind.name()
            },
            chain: chain_name(plan.chain),
            ticker: coin.ticker(),
            wallet_name,
            to: to.clone(),
            to_known,
            to_name,
            lookalike,
            amount: coin.format_amount(amount),
            fee: coin.format_amount(plan.fee),
            fee_rate: fee_rate.filter(|_| plan.chain == Chain::Bitcoin),
            total: coin.format_amount(amount + plan.fee),
            outputs,
            credited: (kind == Kind::Deposit)
                .then(|| vm_fee.map(|f| coin.format_amount(amount.saturating_sub(f))))
                .flatten(),
            confirmations: (kind == Kind::Deposit).then_some(tiers).flatten(),
            payout_fee: (kind == Kind::Withdraw)
                .then(|| info.as_ref().and_then(|i| i.payout_fee.clone()))
                .flatten(),
            previous_fee: None,
        };
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        *self.pending.guard() = Some(Pending {
            id,
            wallet,
            kind,
            plan,
            to,
            amount,
            replaces,
        });
        Review { id, ..review }
    }

    /// Plans a stalled Bitcoin payment's replacement with a higher fee, for
    /// review: the same coins and the same payment, the extra fee from the
    /// change. Its transaction, and those that made its coins, are checked
    /// against their ids.
    pub fn prepare_bump(&self, txid: &str, fee_rate: u64) -> Outcome<Review> {
        let bridge = self.verified(Coin::Btc)?;
        let (id, from) = self.own(Coin::Btc)?;
        let record = self
            .record(&id)
            .and_then(|r| {
                r.outgoing
                    .into_iter()
                    .find(|o| o.txid == txid && o.chain == "bitcoin")
            })
            .map_or_else(|| fail("that payment isn't waiting any more"), Ok)?;
        let api = self.bridge(Coin::Btc);
        let untrusted = |message: String| Failure {
            message,
            untrusted: true,
            canceled: false,
        };
        let raw = hex::decode(api.raw_tx(Chain::Bitcoin, txid)?.trim())
            .map_err(|_| untrusted(format!("transaction {txid} is not hex")))?;
        let original = parse_tx(&raw)?;
        if original.txid() != txid {
            return Err(untrusted(format!(
                "the server sent the wrong transaction for {txid}"
            )));
        }
        let mut raws = HashMap::new();
        for (prev, _) in &original.inputs {
            if !raws.contains_key(prev) {
                raws.insert(prev.clone(), api.raw_tx(Chain::Bitcoin, prev)?);
            }
        }
        let plan = plan_bump(&from, &raw, &raws, fee_rate)?;
        let paid: u64 = original.outputs.iter().map(|o| o.value).sum();
        let previous_fee = plan.total_in.saturating_sub(paid);
        let kind = if record.kind == "deposit" {
            Kind::Deposit
        } else {
            Kind::Send
        };
        let review = self.review(
            kind,
            id,
            plan,
            record.to,
            record.amount,
            &bridge,
            Some(fee_rate),
            Some(txid.to_string()),
        );
        Ok(Review {
            previous_fee: Some(format_btc(previous_fee)),
            ..review
        })
    }

    pub fn cancel(&self, id: u64) {
        let mut pending = self.pending.guard();
        if pending.as_ref().is_some_and(|p| p.id == id) {
            *pending = None;
        }
    }

    /// Signs the reviewed payment after Windows Hello and sends it.
    pub fn confirm(&self, id: u64) -> Outcome<Sent> {
        // Kept until signed, so canceling Windows Hello lets the user retry.
        let pending = match self.pending.guard().as_ref() {
            Some(p) if p.id == id => p.clone(),
            _ => return fail("that payment is no longer waiting; prepare it again"),
        };
        let coin = pending.plan.chain.coin();
        let (wallet, own) = self.own(coin)?;
        if wallet != pending.wallet {
            return fail("the active wallet changed; prepare the payment again");
        }
        let secret = self.vault(&wallet).unlock_secret()?;
        self.note_doge_address(&wallet, &secret);
        let key = secret.key_for(coin)?;
        drop(secret);
        if key.destination_for(coin) != own {
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
        // A replacement's original stays until the network takes this one.
        self.update_record(&wallet, |r| {
            r.outgoing.retain(|o| o.txid != signed.txid);
            r.outgoing.insert(
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
            r.outgoing.truncate(50);
            if pending.kind == Kind::Withdraw {
                r.withdrawals.insert(
                    0,
                    Withdrawal {
                        coin: coin_name(chain.coin()),
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
                r.withdrawals.truncate(50);
            }
        });
        let result = match self.bridge(coin).broadcast(chain, &signed.hex) {
            Ok(txid) if txid == signed.txid => {
                if let Some(original) = &pending.replaces {
                    self.update_record(&wallet, |r| r.outgoing.retain(|o| &o.txid != original));
                    journal::info(&format!("replaced {original} with {txid}"));
                }
                Ok(Sent {
                    txid,
                    chain: chain_name(chain),
                })
            }
            Ok(txid) => Err(Failure {
                message: format!(
                    "the bridge reported transaction {txid}, but this wallet signed {}",
                    signed.txid
                ),
                untrusted: true,
                canceled: false,
            }),
            Err(e) if e.status == 400 => {
                // Refused, so nothing was sent: its coins are free again, or
                // for a replacement, still the original's.
                self.update_record(&wallet, |r| {
                    r.outgoing.retain(|o| o.txid != signed.txid);
                    r.withdrawals.retain(|w| w.txid != signed.txid);
                });
                Err(e.into())
            }
            Err(e) => fail(format!(
                "{}. It may or may not have been sent: it stays in the list of payments in flight, \
                 and its coins aren't offered again until it settles. Its id is {}.",
                e.message, signed.txid
            )),
        };
        match &result {
            Ok(sent) => journal::info(&format!(
                "sent a {} on {}: {}",
                pending.kind.name(),
                sent.chain,
                sent.txid
            )),
            Err(e) => journal::error(&format!("sending {} failed: {}", signed.txid, e.message)),
        }
        result
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
        let side = self.side(Coin::Btc);
        *side.bridge.guard() = (self.connect)(url);
        *side.snapshot.guard() = Snapshot::default();
        *self.pending.guard() = None;
        self.update(|s| s.server = (url != DEFAULT_SERVER).then(|| url.to_string()));
        self.refresh();
        Ok(self.view())
    }

    pub fn set_language(&self, language: &str) -> View {
        let language = if language == "es" { "es" } else { "en" };
        *self.language.guard() = language.into();
        self.update(|s| s.language = Some(language.into()));
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
        let address = |coin: Coin| {
            let net = coin.network();
            Ok::<_, Failure>(decode_address(arg, &net)?.address(&net))
        };
        Ok(match kind {
            "website" => about::WEBSITE.into(),
            "vote" => about::XPR_VOTE_URL.into(),
            "metal" => about::METAL_VALIDATOR_URL.into(),
            "x" => about::X_URL.into(),
            "source" => about::SOURCE_URL.into(),
            "btcvm" => "https://metalbtc.com".into(),
            "dogecoinvm" => DOGE_SERVER.into(),
            "tx-bitcoin" => format!("https://mempool.space/tx/{}", txid()?),
            "tx-btcvm" => format!("https://metalbtc.com/explorer#/tx/{}", txid()?),
            "tx-dogecoin" => format!("https://blockchair.com/dogecoin/transaction/{}", txid()?),
            "tx-dogecoinvm" => format!("{DOGE_SERVER}/explorer#/tx/{}", txid()?),
            "address-bitcoin" => format!("https://mempool.space/address/{}", address(Coin::Btc)?),
            "address-btcvm" => format!(
                "https://metalbtc.com/explorer#/address/{}",
                address(Coin::Btc)?
            ),
            "address-dogecoin" => format!(
                "https://blockchair.com/dogecoin/address/{}",
                address(Coin::Doge)?
            ),
            "address-dogecoinvm" => {
                format!("{DOGE_SERVER}/explorer#/address/{}", address(Coin::Doge)?)
            }
            "check" => {
                let coin = self.active_coin();
                let snap = self.side(coin).snapshot.guard();
                let links = self
                    .bridge_view(&snap, coin)
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

/// `count` different numbers below `n`, in order, from the system's
/// generator.
fn pick(n: usize, count: usize) -> Vec<usize> {
    use rand_core::RngCore;
    let mut picked: Vec<usize> = Vec::with_capacity(count);
    while picked.len() < count.min(n) {
        let i = rand_core::OsRng.next_u32() as usize % n;
        if !picked.contains(&i) {
            picked.push(i);
        }
    }
    picked.sort_unstable();
    picked
}

/// Random bytes for a wallet's id, from the system's generator.
fn rand_id(buf: &mut [u8]) {
    use rand_core::RngCore;
    rand_core::OsRng.fill_bytes(buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use btcvm_wallet_core::parse_btc;

    fn deposit(
        amount: &str,
        status: &str,
        credited: Option<&str>,
        credit_txid: bool,
    ) -> DepositEntry {
        DepositEntry {
            txid: "aa".repeat(32),
            vout: 0,
            amount: amount.into(),
            confirmations: 1,
            required: 2,
            status: status.into(),
            reason: None,
            credited: credited.map(Into::into),
            credit_txid: credit_txid.then(|| "bb".repeat(32)),
            refund_txid: None,
        }
    }

    #[test]
    fn a_stored_rotation_that_doesnt_check_out_is_ignored() {
        let pinned = Pinned::mainnet().signers;
        let other = Signers {
            required: 1,
            public_keys: vec![pinned.public_keys[0].clone()],
        };
        let forged = RotationProof {
            chain: "btcvm".into(),
            from: pinned.clone(),
            to: other,
            tx: "00".into(),
            prev: Default::default(),
            verified: 0,
        };
        assert_eq!(replay(&pinned, &[]), pinned);
        assert_eq!(replay(&pinned, &[forged]), pinned);
    }

    #[test]
    fn deposits_on_their_way_are_pending_on_btcvm() {
        let info: BridgeInfo = serde_json::from_str(include_str!(
            "../../core/tests/fixtures/metalbtc-info-2026-10-03.json"
        ))
        .unwrap();
        let fee = parse_btc(info.vm_fee.as_deref().unwrap()).unwrap();
        let deposits = [
            deposit("0.00019847", "confirming", None, false),
            deposit("0.0001", "waiting_for_capacity", None, false),
            deposit("0.0002", "crediting", Some("0.00019"), false),
            // Credited, or with its credit on BTCVM already: in the balance.
            deposit("0.0003", "crediting", Some("0.00029"), true),
            deposit("0.0004", "credited", Some("0.00039"), true),
            // Not coming: held or refunded.
            deposit("0.0005", "held", None, false),
            deposit("0.0006", "refunded", None, false),
        ];
        let expected = 19_847 - fee + 10_000 - fee + 19_000;
        let facts = Facts::from(&info);
        assert_eq!(
            incoming(&deposits, Some(&facts), Coin::Btc),
            Some(format_btc(expected))
        );
        assert_eq!(incoming(&deposits[3..], Some(&facts), Coin::Btc), None);
        assert_eq!(incoming(&[], None, Coin::Btc), None);
    }

    #[test]
    fn dogecoin_deposits_on_their_way_are_pending_on_dogecoinvm() {
        let info: DogeBridgeInfo = serde_json::from_str(include_str!(
            "../../core/tests/fixtures/metaldoge-info-2026-10-06.json"
        ))
        .unwrap();
        let facts = Facts::from(&info);
        // DogecoinVM keeps 0.01 DOGE of each deposit.
        let deposits = [
            deposit("25", "confirming", None, false),
            deposit("10", "crediting", Some("9.99"), false),
        ];
        assert_eq!(
            incoming(&deposits, Some(&facts), Coin::Doge).as_deref(),
            Some("34.98")
        );
        assert_eq!(facts.payout_fee.as_deref(), Some("0.10000000"));
        assert!(facts.l1_wallet);
    }
}
