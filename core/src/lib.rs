//! Keys, addresses, bridge checks and transactions for Bitcoin and BTCVM.
//!
//! This is the Windows wallet's counterpart of the BTCVM web wallet's
//! `chain.js` (MetalBlockchain/btc-vm, `cmd/btcvm/web`), and it must agree
//! with it byte for byte: `tests/vectors.rs` checks it against vectors the
//! web wallet generated and btcd's script engine verified.
//!
//! On top of what the web wallet does, it doesn't take the bridge's word for
//! where coins go. The peg's signer set is pinned ([`bridge::Pinned`]), and
//! a deposit address or a withdrawal that doesn't follow from it is refused.
//! A dishonest or hijacked server can stall the wallet, but it can't redirect
//! a payment, inflate a fee or get anything signed that wasn't reviewed.
//!
//! The core does no networking and keeps no state: the app fetches, the core
//! checks, plans and signs. Private keys leave it only for the app's vault.

#![forbid(unsafe_code)]

pub mod about;
pub mod address;
pub mod amount;
pub mod bridge;
pub mod encoding;
pub mod keys;
pub mod payment;
pub mod tx;
pub mod wallet;

pub use address::{Destination, Kind, MAINNET, Network, TESTNET, decode_address};
pub use amount::{SATS_PER_BTC, format_btc, parse_btc};
pub use bridge::{BridgeInfo, Pinned, SignerChange, Signers, VerifiedBridge};
pub use keys::Key;
pub use payment::{
    Chain, OutputView, Plan, Request, Signed, Utxo, build_payment, plan_payment, sign_plan,
};
pub use wallet::{Coins, plan_deposit, plan_send, plan_withdrawal};

/// Why the core refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Something given to the core is wrong or not enough: a typo in an
    /// address, an amount out of range, too few coins.
    Invalid(String),
    /// A server answer the core can check turned out false. Nothing was
    /// signed; the app should say so plainly, as it may be an attack.
    Untrusted(String),
}

impl Error {
    pub fn message(&self) -> &str {
        match self {
            Error::Invalid(m) | Error::Untrusted(m) => m,
        }
    }

    pub fn is_untrusted(&self) -> bool {
        matches!(self, Error::Untrusted(_))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn invalid<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Invalid(msg.into()))
}

pub(crate) fn untrusted<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Untrusted(msg.into()))
}
