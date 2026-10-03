//! The bridge: the peg's signer set and the scripts built from it, the BVMO
//! withdrawal tag, and the check of what the bridge says about itself
//! against the signer set this wallet trusts.

use crate::address::{Destination, Kind, MAINNET, Network, decode_address, push_data};
use crate::amount::parse_btc;
use crate::encoding::sha256;
use crate::payment::MAX_FEE_RATE;
use crate::{Error, Result, invalid, untrusted};
use std::collections::HashSet;

/// The peg's signers: an m-of-n multisig of compressed public keys, in the
/// order the bridge lists them (btcd's `MultiSigScript` keeps that order).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Signers {
    pub required: u8,
    pub public_keys: Vec<String>,
}

impl Signers {
    /// `OP_m <keys…> OP_n OP_CHECKMULTISIG`: the witness script that locks
    /// BTC on Bitcoin and the reserve on BTCVM.
    pub fn witness_script(&self) -> Result<Vec<u8>> {
        let n = self.public_keys.len();
        if self.required < 1 || usize::from(self.required) > n || n > 15 {
            return invalid("invalid signer set");
        }
        let mut seen = HashSet::new();
        let mut s = vec![0x50 + self.required];
        for k in &self.public_keys {
            let raw = hex::decode(k).map_err(|_| Error::Invalid("signer key is not hex".into()))?;
            if raw.len() != 33 || k256::PublicKey::from_sec1_bytes(&raw).is_err() {
                return invalid("signer keys must be compressed public keys");
            }
            s.extend(push_data(&raw)?);
            if !seen.insert(raw) {
                return invalid("a signer key appears twice");
            }
        }
        s.push(0x50 + n as u8);
        s.push(0xae);
        Ok(s)
    }

    /// The peg: P2WSH of the witness script. The reserve on BTCVM and the
    /// locked BTC on Bitcoin share this one `bc1q…` address.
    pub fn peg(&self) -> Result<Destination> {
        Destination::new(Kind::P2wsh, &sha256(&self.witness_script()?))
    }

    /// The witness script of `dest`'s personal deposit address: `<kind ||
    /// program> OP_DROP`, then the signers' multisig. Only the signers can
    /// spend it, and the bridge credits `dest` on BTCVM.
    pub fn deposit_script(&self, dest: &Destination) -> Result<Vec<u8>> {
        Ok([
            push_data(&dest.tag_bytes())?,
            vec![0x75],
            self.witness_script()?,
        ]
        .concat())
    }

    /// `dest`'s personal Bitcoin deposit address (P2WSH), computed from the
    /// signers' keys so the wallet can check what the bridge says.
    pub fn deposit_destination(&self, dest: &Destination) -> Result<Destination> {
        Destination::new(Kind::P2wsh, &sha256(&self.deposit_script(dest)?))
    }
}

/// Tags the OP_RETURN of a withdrawal (peg-out): the reserve pays the
/// destination that follows on Bitcoin.
pub const PEG_OUT_TAG: &[u8; 4] = b"BVMO";

/// The BVMO tag asking the bridge to pay a withdrawal to `btc` on Bitcoin.
pub fn peg_out_data(btc: &Destination) -> Vec<u8> {
    [&PEG_OUT_TAG[..], &btc.tag_bytes()].concat()
}

/// The Bitcoin destination a BVMO tag names, or None if `data` isn't one.
pub fn peg_out_destination(data: &[u8]) -> Option<Destination> {
    data.strip_prefix(&PEG_OUT_TAG[..])
        .and_then(Destination::from_tag_bytes)
}

/// Address encodings as the bridge reports them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Versions {
    pub hrp: String,
    pub p2pkh: u8,
    pub p2sh: u8,
    pub wif: u8,
}

/// Smaller deposits need fewer Bitcoin confirmations.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationTier {
    /// BTC, as a decimal string.
    pub up_to: String,
    pub confirmations: u32,
}

/// The bridge's `GET /api/info`, as far as the wallet uses it. Nothing in it
/// is trusted until [`verify`] has checked it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInfo {
    pub bitcoin_network: String,
    pub btcvm_network: String,
    pub bitcoin_versions: Versions,
    pub btcvm_versions: Versions,
    #[serde(rename = "chainID")]
    pub chain_id: String,
    pub peg_address: String,
    pub reserve_address: String,
    pub signers: Signers,
    /// The bridge's Bitcoin fee estimate, sat/vB.
    pub btc_fee_rate: u64,
    pub min_deposit: String,
    pub max_deposit: String,
    pub min_peg_out: String,
    pub max_circulating: String,
    pub deposit_confirmations: u32,
    #[serde(default)]
    pub confirmation_tiers: Vec<ConfirmationTier>,
    /// Whether the bridge serves Bitcoin balances too.
    #[serde(default)]
    pub btc_wallet: bool,
    /// What the bridge keeps from a deposit, in BTC (the BTCVM fee of the
    /// release that credits it).
    #[serde(default)]
    pub vm_fee: Option<String>,
    /// What a withdrawal's payout costs on Bitcoin at the current rate, in
    /// BTC, taken from the amount paid.
    #[serde(default)]
    pub payout_fee: Option<String>,
}

/// What this wallet trusts about a bridge, fixed when the wallet is built:
/// its networks, its chain and the peg's signer set. A bridge on other
/// networks is refused outright. One reporting another signer set is
/// accepted for sends only, with a [`SignerChange`] to warn about, so a
/// hijacked server or connection can't swap in its own keys and collect
/// deposits or withdrawals.
#[derive(Debug, Clone)]
pub struct Pinned {
    pub net: Network,
    pub bitcoin_network: &'static str,
    pub btcvm_network: &'static str,
    pub chain_id: &'static str,
    pub signers: Signers,
}

/// The BTCVM mainnet bridge at metalbtc.com, as its `/api/info` reported it
/// on 2026-10-03. The peg address holds the locked BTC on Bitcoin.
pub mod mainnet {
    pub const BITCOIN_NETWORK: &str = "mainnet";
    pub const BTCVM_NETWORK: &str = "btcvm";
    pub const CHAIN_ID: &str = "BYogm85qvZxwX4PitKLDPzNDbAgo61nw2NSXx5VVXyZZ8yGUK";
    pub const SIGNERS_REQUIRED: u8 = 2;
    pub const SIGNER_KEYS: [&str; 3] = [
        "03551efcbbab205134366745320d969682e53a8495bd29554a93415d6f4b24ace9",
        "03cb2d10adea7ceeeea227e0ef87225f1b72aa25c2cd565e2bc27e699e0396fc66",
        "034ee78c8cf5393f2a6e998da0279a70e0fb53c62e7d49228b964f48e737096256",
    ];
    /// The peg address those keys make. A test checks they agree, so a
    /// mistyped key can't slip in.
    pub const PEG_ADDRESS: &str = "bc1qaunaxg4hu96ja6pvarfuph663lx96xau5dkhxcsg2ypve6n6wwlsvew3h6";
}

impl Pinned {
    pub fn mainnet() -> Pinned {
        Pinned {
            net: MAINNET,
            bitcoin_network: mainnet::BITCOIN_NETWORK,
            btcvm_network: mainnet::BTCVM_NETWORK,
            chain_id: mainnet::CHAIN_ID,
            signers: Signers {
                required: mainnet::SIGNERS_REQUIRED,
                public_keys: mainnet::SIGNER_KEYS.iter().map(|k| k.to_string()).collect(),
            },
        }
    }
}

/// A bridge whose `/api/info` matched what the wallet trusts: the scripts
/// and limits to use.
#[derive(Debug, Clone)]
pub struct VerifiedBridge {
    pub net: Network,
    /// The pinned signer set.
    pub signers: Signers,
    /// The peg the pinned set makes: the reserve on BTCVM and the locked BTC
    /// on Bitcoin.
    pub peg: Destination,
    /// Set when the bridge reports another signer set. Deposits and
    /// withdrawals are then refused until the wallet is updated; sends on
    /// either chain still work.
    pub signer_change: Option<SignerChange>,
    pub min_deposit: u64,
    /// 0 when there is no cap.
    pub max_deposit: u64,
    pub min_peg_out: u64,
    /// sat/vB, within the wallet's own bounds.
    pub btc_fee_rate: u64,
}

impl VerifiedBridge {
    /// Whether `dest` is the bridge's own address, pinned or newly reported.
    /// Coins paid there without a tag would sit unclaimed.
    pub fn is_peg(&self, dest: &Destination) -> bool {
        *dest == self.peg
            || self
                .signer_change
                .as_ref()
                .is_some_and(|c| *dest == c.reported_peg)
    }
}

/// The bridge reports another signer set than the one pinned in the wallet,
/// and its peg and reserve addresses follow from that set.
///
/// That is what a rotation by BTCVM's operators looks like
/// (`docs/ROTATION.md` in btc-vm), and also what a hijacked server would
/// show; the wallet can't tell them apart on its own. So it moves nothing
/// between the chains until it is updated with the new set, and the app
/// warns, showing what changed and where to check it without relying on the
/// bridge's server. This is deliberate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignerChange {
    pub trusted: Signers,
    pub trusted_peg: Destination,
    pub reported: Signers,
    pub reported_peg: Destination,
}

/// Somewhere to check a signer change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSource {
    /// The old peg address on a Bitcoin explorer. After a real rotation its
    /// BTC has moved to the new peg, which only the old signers could do.
    OldPegOnBitcoin,
    /// The new peg address on a Bitcoin explorer.
    NewPegOnBitcoin,
    /// BTCVM's documentation at metalbtc.com, run by the same operators as
    /// the bridge's API.
    BtcvmDocs,
    /// BTCVM's explorer, which links each locked output to Bitcoin.
    BtcvmExplorer,
    /// How BTCVM's operators rotate the signer set.
    RotationProcedure,
    /// madblocks, who publish each signer set they have verified with the
    /// wallet's updates.
    WalletMaker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckLink {
    pub source: CheckSource,
    pub url: String,
}

impl SignerChange {
    /// Keys the bridge now lists that the trusted set doesn't have.
    pub fn added_keys(&self) -> Vec<String> {
        key_difference(&self.reported, &self.trusted)
    }

    /// Keys of the trusted set the bridge no longer lists.
    pub fn removed_keys(&self) -> Vec<String> {
        key_difference(&self.trusted, &self.reported)
    }

    /// Where to check the change, most independent of the bridge first.
    pub fn where_to_check(&self, net: &Network) -> Vec<CheckLink> {
        let link = |source, url: String| CheckLink { source, url };
        let mut links = Vec::new();
        if net.hrp == MAINNET.hrp {
            let explorer =
                |d: &Destination| format!("https://mempool.space/address/{}", d.address(net));
            links.push(link(
                CheckSource::OldPegOnBitcoin,
                explorer(&self.trusted_peg),
            ));
            links.push(link(
                CheckSource::NewPegOnBitcoin,
                explorer(&self.reported_peg),
            ));
        }
        links.extend([
            link(
                CheckSource::RotationProcedure,
                "https://github.com/MetalBlockchain/btc-vm/blob/main/docs/ROTATION.md".into(),
            ),
            link(CheckSource::BtcvmDocs, "https://metalbtc.com/docs".into()),
            link(
                CheckSource::BtcvmExplorer,
                "https://metalbtc.com/explorer".into(),
            ),
            link(CheckSource::WalletMaker, crate::about::WEBSITE.into()),
        ]);
        links
    }
}

/// The keys of `a` that `b` doesn't have, compared without regard to case.
fn key_difference(a: &Signers, b: &Signers) -> Vec<String> {
    let theirs: HashSet<String> = b.public_keys.iter().map(|k| k.to_lowercase()).collect();
    a.public_keys
        .iter()
        .filter(|k| !theirs.contains(&k.to_lowercase()))
        .cloned()
        .collect()
}

/// Checks the bridge's `/api/info` against what the wallet trusts.
pub fn verify(info: &BridgeInfo, pinned: &Pinned) -> Result<VerifiedBridge> {
    if info.bitcoin_network != pinned.bitcoin_network
        || info.btcvm_network != pinned.btcvm_network
        || info.chain_id != pinned.chain_id
    {
        return untrusted(format!(
            "the bridge is for {} and {} (chain {}), not the networks this wallet is for",
            info.bitcoin_network, info.btcvm_network, info.chain_id
        ));
    }
    let net = &pinned.net;
    for v in [&info.bitcoin_versions, &info.btcvm_versions] {
        if v.hrp != net.hrp || v.p2pkh != net.p2pkh || v.p2sh != net.p2sh || v.wif != net.wif {
            return untrusted("the bridge reports address formats that aren't the network's");
        }
    }
    // The bridge's own story must hold together: its peg and reserve are
    // the addresses its signers make. Only then is a signer set other than
    // the pinned one a change to warn about, rather than a broken bridge.
    let peg = pinned.signers.peg()?;
    let reported = info
        .signers
        .witness_script()
        .map_err(|e| Error::Untrusted(format!("the bridge's signer set is invalid: {e}")))?;
    let changed = reported != pinned.signers.witness_script()?;
    let reported_peg = if changed {
        info.signers.peg()?
    } else {
        peg.clone()
    };
    for (name, address) in [
        ("peg", &info.peg_address),
        ("reserve", &info.reserve_address),
    ] {
        if decode_address(address, net).ok().as_ref() != Some(&reported_peg) {
            return untrusted(format!(
                "the bridge's {name} address doesn't follow from its signers"
            ));
        }
    }
    let signer_change = changed.then(|| SignerChange {
        trusted: pinned.signers.clone(),
        trusted_peg: peg.clone(),
        reported: info.signers.clone(),
        reported_peg,
    });
    let amount = |name: &str, s: &str| {
        parse_btc(s).map_err(|_| Error::Untrusted(format!("the bridge's {name} isn't an amount")))
    };
    let min_deposit = amount("minimum deposit", &info.min_deposit)?;
    let max_deposit = amount("maximum deposit", &info.max_deposit)?;
    let min_peg_out = amount("minimum withdrawal", &info.min_peg_out)?;
    if max_deposit > 0 && min_deposit > max_deposit {
        return untrusted("the bridge's deposit limits contradict each other");
    }
    if !(1..=MAX_FEE_RATE).contains(&info.btc_fee_rate) {
        return untrusted(format!(
            "the bridge's fee rate, {} sat/vB, looks wrong",
            info.btc_fee_rate
        ));
    }
    Ok(VerifiedBridge {
        net: *net,
        signers: pinned.signers.clone(),
        peg,
        signer_change,
        min_deposit,
        max_deposit,
        min_peg_out,
        btc_fee_rate: info.btc_fee_rate,
    })
}
