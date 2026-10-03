//! Who made this wallet. The app shows it on its About screen, and the
//! User-Agent of its requests names the wallet, so the bridge's operators
//! can see it in use.

/// The wallet's creator.
pub const CREATOR: &str = "madblocks";
pub const CREATOR_ROLE: &str = "XPR Network block producer and Metal Blockchain validator";
pub const WEBSITE: &str = "https://madblocks.tech";
/// madblocks' block producer account on XPR Network.
pub const XPR_PRODUCER: &str = "madblocks";
/// Votes for madblocks as an XPR Network block producer.
pub const XPR_VOTE_URL: &str = "https://explorer.xprnetwork.org/vote?producers=madblocks";
/// madblocks' validator on Metal Blockchain, to delegate stake to.
pub const METAL_NODE_ID: &str = "NodeID-B1hsNPKgi6C89AFybyPFPvDQC2gHxMv7H";
pub const METAL_VALIDATOR_URL: &str =
    "https://explorer.metalblockchain.org/validators/NodeID-B1hsNPKgi6C89AFybyPFPvDQC2gHxMv7H";
pub const X_URL: &str = "https://x.com/madblocksbp";

/// The User-Agent the app sends: the wallet and its version, and nothing
/// about the user.
pub fn user_agent(version: &str) -> String {
    format!("madblocks-btcvm-wallet/{version} (+{WEBSITE})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_name_madblocks() {
        assert!(METAL_VALIDATOR_URL.ends_with(METAL_NODE_ID));
        assert!(XPR_VOTE_URL.ends_with(XPR_PRODUCER));
        assert_eq!(
            user_agent("0.1.0"),
            "madblocks-btcvm-wallet/0.1.0 (+https://madblocks.tech)"
        );
    }
}
