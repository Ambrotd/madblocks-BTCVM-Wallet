//! The bridge's public API (`btcvm serve`), the one metalbtc.com's web wallet
//! uses. It holds no keys, and nothing it says moves coins unchecked: the
//! core verifies what matters before anything is signed.

use btcvm_wallet_core::{BridgeInfo, Chain, Utxo};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{BufRead, BufReader, Read};
use std::time::Duration;

pub const DEFAULT_SERVER: &str = "https://metalbtc.com";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub txid: String,
    /// Signed BTC.
    pub net: String,
    pub confirmations: i64,
    pub time: Option<i64>,
}

/// An address on one chain: its balance, coins and recent history.
#[derive(Debug, Clone, Deserialize)]
pub struct AddressView {
    pub confirmed: String,
    pub pending: String,
    pub utxos: Vec<Utxo>,
    pub history: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositEntry {
    pub txid: String,
    pub vout: u32,
    pub amount: String,
    pub confirmations: i64,
    pub required: i64,
    /// confirming, waiting_for_capacity, crediting, credited, held or refunded.
    pub status: String,
    pub reason: Option<String>,
    pub credited: Option<String>,
    pub credit_txid: Option<String>,
    pub refund_txid: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PegOutStatus {
    /// pending, paid or unknown.
    pub status: String,
    pub pays: Option<String>,
    pub payment_txid: Option<String>,
    pub payment_confirmations: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatus {
    pub btcvm_height: Option<i64>,
    pub bitcoin_height: Option<i64>,
    pub bitcoin_sync: Option<SyncState>,
    /// Set while the operators have paused the bridge.
    pub paused: Option<serde_json::Value>,
    pub audit: Option<Audit>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SyncState {
    pub syncing: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Audit {
    pub locked: String,
    pub circulating: String,
    pub solvent: bool,
}

/// Bitcoin fee rates, sat/vB, as mempool.space recommends them.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeEstimates {
    pub fastest_fee: f64,
    pub half_hour_fee: f64,
    pub hour_fee: f64,
    pub economy_fee: f64,
    pub minimum_fee: f64,
}

/// A transaction as mempool.space lists an address's: its id and output
/// scripts, enough to spot a BVMM tag.
#[derive(Debug, Clone, Deserialize)]
pub struct ListedTx {
    pub txid: String,
    pub vout: Vec<ListedOut>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListedOut {
    /// Hex.
    pub scriptpubkey: String,
}

/// BTC's price in dollars and euros, from mempool.space.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Prices {
    #[serde(rename = "USD")]
    pub usd: f64,
    #[serde(rename = "EUR")]
    pub eur: f64,
}

#[derive(Debug, Clone)]
pub struct ApiError {
    /// 0 when there was no answer.
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// What the wallet asks of the bridge's server, and of mempool.space: a
/// trait, so tests can stand in for the network.
pub trait Api: Send + Sync {
    fn base(&self) -> &str;
    fn info(&self) -> Result<BridgeInfo, ApiError>;
    fn status(&self) -> Result<BridgeStatus, ApiError>;
    fn address(&self, chain: Chain, address: &str) -> Result<AddressView, ApiError>;
    fn watch_bitcoin(&self, address: &str) -> Result<(), ApiError>;
    fn raw_tx(&self, chain: Chain, txid: &str) -> Result<String, ApiError>;
    fn broadcast(&self, chain: Chain, hex: &str) -> Result<String, ApiError>;
    fn deposit_address(&self, address: &str) -> Result<String, ApiError>;
    fn deposits(&self, address: &str) -> Result<Vec<DepositEntry>, ApiError>;
    fn peg_out(&self, txid: &str) -> Result<PegOutStatus, ApiError>;
    fn events(&self) -> Result<Box<dyn BufRead + Send>, ApiError>;
    fn recommended_fees(&self) -> Result<FeeEstimates, ApiError>;
    fn prices(&self) -> Result<Prices, ApiError>;
    fn bitcoin_address_txs(&self, address: &str) -> Result<Vec<ListedTx>, ApiError>;
    fn bitcoin_balance(&self, address: &str) -> Result<u64, ApiError>;
}

impl Api for Bridge {
    fn base(&self) -> &str {
        Bridge::base(self)
    }
    fn info(&self) -> Result<BridgeInfo, ApiError> {
        Bridge::info(self)
    }
    fn status(&self) -> Result<BridgeStatus, ApiError> {
        Bridge::status(self)
    }
    fn address(&self, chain: Chain, address: &str) -> Result<AddressView, ApiError> {
        Bridge::address(self, chain, address)
    }
    fn watch_bitcoin(&self, address: &str) -> Result<(), ApiError> {
        Bridge::watch_bitcoin(self, address)
    }
    fn raw_tx(&self, chain: Chain, txid: &str) -> Result<String, ApiError> {
        Bridge::raw_tx(self, chain, txid)
    }
    fn broadcast(&self, chain: Chain, hex: &str) -> Result<String, ApiError> {
        Bridge::broadcast(self, chain, hex)
    }
    fn deposit_address(&self, address: &str) -> Result<String, ApiError> {
        Bridge::deposit_address(self, address)
    }
    fn deposits(&self, address: &str) -> Result<Vec<DepositEntry>, ApiError> {
        Bridge::deposits(self, address)
    }
    fn peg_out(&self, txid: &str) -> Result<PegOutStatus, ApiError> {
        Bridge::peg_out(self, txid)
    }
    fn events(&self) -> Result<Box<dyn BufRead + Send>, ApiError> {
        Ok(Box::new(Bridge::events(self)?))
    }
    fn recommended_fees(&self) -> Result<FeeEstimates, ApiError> {
        Bridge::recommended_fees(self)
    }
    fn prices(&self) -> Result<Prices, ApiError> {
        Bridge::prices(self)
    }
    fn bitcoin_address_txs(&self, address: &str) -> Result<Vec<ListedTx>, ApiError> {
        Bridge::bitcoin_address_txs(self, address)
    }
    fn bitcoin_balance(&self, address: &str) -> Result<u64, ApiError> {
        Bridge::bitcoin_balance(self, address)
    }
}

pub struct Bridge {
    base: String,
    agent: ureq::Agent,
}

impl Bridge {
    pub fn new(base: &str) -> Bridge {
        Bridge {
            base: base.trim_end_matches('/').to_string(),
            agent: agent(Some(Duration::from_secs(30))),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn info(&self) -> Result<BridgeInfo, ApiError> {
        self.get("/api/info")
    }

    pub fn status(&self) -> Result<BridgeStatus, ApiError> {
        self.get("/api/status")
    }

    /// The address on `chain`. On Bitcoin the bridge answers 404 until the
    /// address is registered with [`Bridge::watch_bitcoin`].
    pub fn address(&self, chain: Chain, address: &str) -> Result<AddressView, ApiError> {
        check_address(address)?;
        self.get(&match chain {
            Chain::Btcvm => format!("/api/address/{address}"),
            Chain::Bitcoin => format!("/api/btc/address/{address}"),
        })
    }

    pub fn watch_bitcoin(&self, address: &str) -> Result<(), ApiError> {
        let _: serde_json::Value = self.post("/api/btc/watch", json!({ "address": address }))?;
        Ok(())
    }

    /// The bytes of a transaction, hex. The caller checks them against the
    /// txid. A pruned Bitcoin node may no longer have an old one; then it
    /// comes from a public explorer, as the web wallet does.
    pub fn raw_tx(&self, chain: Chain, txid: &str) -> Result<String, ApiError> {
        check_txid(txid)?;
        #[derive(Deserialize)]
        struct Hex {
            hex: String,
        }
        let path = match chain {
            Chain::Btcvm => format!("/api/rawtx/{txid}"),
            Chain::Bitcoin => format!("/api/btc/rawtx/{txid}"),
        };
        match self.get::<Hex>(&path) {
            Ok(h) => Ok(h.hex),
            Err(e) if chain == Chain::Bitcoin && e.status == 404 => {
                let mut last = e;
                for base in [
                    "https://mempool.space/api/tx/",
                    "https://blockstream.info/api/tx/",
                ] {
                    match self.agent.get(format!("{base}{txid}/hex")).call() {
                        Ok(mut r) if r.status().is_success() => {
                            return r
                                .body_mut()
                                .read_to_string()
                                .map(|s| s.trim().to_string())
                                .map_err(network);
                        }
                        Ok(r) => {
                            last = ApiError {
                                status: r.status().as_u16(),
                                message: format!("no transaction {txid}"),
                            }
                        }
                        Err(e) => last = network(e),
                    }
                }
                Err(last)
            }
            Err(e) => Err(e),
        }
    }

    /// Sends a signed transaction. A 400 means the network refused it and
    /// nothing was sent; any other failure may or may not have sent it.
    pub fn broadcast(&self, chain: Chain, hex: &str) -> Result<String, ApiError> {
        #[derive(Deserialize)]
        struct Sent {
            txid: String,
        }
        let path = match chain {
            Chain::Btcvm => "/api/tx",
            Chain::Bitcoin => "/api/btc/tx",
        };
        Ok(self.post::<Sent>(path, json!({ "hex": hex }))?.txid)
    }

    /// Registers the wallet's address for deposits and returns the Bitcoin
    /// address the bridge says to pay, for the core to check.
    pub fn deposit_address(&self, address: &str) -> Result<String, ApiError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Registered {
            deposit_address: String,
        }
        Ok(self
            .post::<Registered>("/api/deposit-address", json!({ "address": address }))?
            .deposit_address)
    }

    pub fn deposits(&self, address: &str) -> Result<Vec<DepositEntry>, ApiError> {
        check_address(address)?;
        self.get(&format!("/api/deposits/{address}"))
    }

    pub fn peg_out(&self, txid: &str) -> Result<PegOutStatus, ApiError> {
        check_txid(txid)?;
        self.get(&format!("/api/pegout/{txid}"))
    }

    /// The bridge's stream of new blocks (server-sent events). It is cut and
    /// reopened every few minutes, so a silently dead connection can't last.
    pub fn events(&self) -> Result<impl BufRead + use<>, ApiError> {
        let agent = agent(None);
        let response = agent
            .get(format!("{}/api/events", self.base))
            .header("Accept", "text/event-stream")
            .call()
            .map_err(network)?;
        if !response.status().is_success() {
            return Err(ApiError {
                status: response.status().as_u16(),
                message: "no event stream".into(),
            });
        }
        Ok(BufReader::new(
            response.into_body().into_reader().take(1 << 20),
        ))
    }

    /// mempool.space's recommended Bitcoin fee rates. A third party, used
    /// only to offer choices; the wallet bounds whatever is chosen.
    pub fn recommended_fees(&self) -> Result<FeeEstimates, ApiError> {
        decode(
            self.agent
                .get("https://mempool.space/api/v1/fees/recommended")
                .call()
                .map_err(network)?,
        )
    }

    /// An address's latest transactions on Bitcoin, from mempool.space: to
    /// find the old signers' move after a rotation. What they say is checked
    /// by their signatures, not taken on trust.
    pub fn bitcoin_address_txs(&self, address: &str) -> Result<Vec<ListedTx>, ApiError> {
        check_address(address)?;
        decode(
            self.agent
                .get(format!("https://mempool.space/api/address/{address}/txs"))
                .call()
                .map_err(network)?,
        )
    }

    /// An address's confirmed balance on Bitcoin according to mempool.space,
    /// in satoshis: a second opinion on what the bridge says.
    pub fn bitcoin_balance(&self, address: &str) -> Result<u64, ApiError> {
        #[derive(Deserialize)]
        struct Stats {
            funded_txo_sum: u64,
            spent_txo_sum: u64,
        }
        #[derive(Deserialize)]
        struct Address {
            chain_stats: Stats,
        }
        check_address(address)?;
        let a: Address = decode(
            self.agent
                .get(format!("https://mempool.space/api/address/{address}"))
                .call()
                .map_err(network)?,
        )?;
        Ok(a.chain_stats
            .funded_txo_sum
            .saturating_sub(a.chain_stats.spent_txo_sum))
    }

    /// BTC's price from mempool.space, to show values in a currency. Nothing
    /// is signed or sized from it.
    pub fn prices(&self) -> Result<Prices, ApiError> {
        decode(
            self.agent
                .get("https://mempool.space/api/v1/prices")
                .call()
                .map_err(network)?,
        )
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        decode(
            self.agent
                .get(format!("{}{path}", self.base))
                .call()
                .map_err(network)?,
        )
    }

    fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<T, ApiError> {
        decode(
            self.agent
                .post(format!("{}{path}", self.base))
                .send_json(body)
                .map_err(network)?,
        )
    }
}

fn agent(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(timeout)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_body(timeout.or(Some(Duration::from_secs(300))))
        .user_agent(btcvm_wallet_core::about::user_agent(env!(
            "CARGO_PKG_VERSION"
        )))
        // Certificates are checked by Windows, as a browser does, so the app
        // works where antivirus or a company proxy inspects TLS. The wallet's
        // safety doesn't rest on TLS: what moves coins is checked by the core.
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .http_status_as_error(false)
        .https_only(true)
        .build()
        .new_agent()
}

fn decode<T: DeserializeOwned>(
    mut response: ureq::http::Response<ureq::Body>,
) -> Result<T, ApiError> {
    let status = response.status().as_u16();
    if (200..300).contains(&status) {
        return response.body_mut().read_json().map_err(|e| ApiError {
            status,
            message: format!("the bridge's answer didn't parse: {e}"),
        });
    }
    #[derive(Deserialize)]
    struct Failure {
        error: String,
    }
    let message = response
        .body_mut()
        .read_json::<Failure>()
        .map(|f| f.error)
        .unwrap_or_else(|_| format!("the bridge answered {status}"));
    Err(ApiError { status, message })
}

fn network(e: ureq::Error) -> ApiError {
    ApiError {
        status: 0,
        message: format!("can't reach the bridge: {e}"),
    }
}

/// Addresses and txids go into URL paths, so only their own characters pass.
fn check_address(address: &str) -> Result<(), ApiError> {
    if address.is_empty()
        || address.len() > 90
        || !address.bytes().all(|c| c.is_ascii_alphanumeric())
    {
        return Err(ApiError {
            status: 0,
            message: "not an address".into(),
        });
    }
    Ok(())
}

fn check_txid(txid: &str) -> Result<(), ApiError> {
    if txid.len() != 64 || !txid.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError {
            status: 0,
            message: "not a transaction id".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs the network"]
    fn reaches_the_live_bridge() {
        let bridge = Bridge::new(DEFAULT_SERVER);
        if let Err(e) = bridge.info() {
            panic!("{} (status {})", e.message, e.status);
        }
    }
}
