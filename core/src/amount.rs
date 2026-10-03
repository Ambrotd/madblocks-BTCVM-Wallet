//! Amounts: satoshis inside, BTC strings for people and for the bridge's
//! API.

use crate::{Result, invalid};

pub const SATS_PER_BTC: u64 = 100_000_000;
/// Bitcoin's whole supply: the most a single output can hold on either
/// chain.
pub const MAX_MONEY: u64 = 21_000_000 * SATS_PER_BTC;

/// Parses a BTC amount like "0.0025", with up to 8 decimals, into satoshis.
pub fn parse_btc(s: &str) -> Result<u64> {
    let s = s.trim();
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    let digits = |t: &str| (1..=8).contains(&t.len()) && t.bytes().all(|c| c.is_ascii_digit());
    if !digits(whole) || frac.is_some_and(|f| !digits(f)) {
        return invalid("enter an amount like 0.0025");
    }
    let whole: u64 = whole.parse().expect("checked digits");
    let frac: u64 = frac.map_or(0, |f| format!("{f:0<8}").parse().expect("checked digits"));
    let sats = whole * SATS_PER_BTC + frac;
    if sats > MAX_MONEY {
        return invalid("more than 21 million BTC");
    }
    Ok(sats)
}

/// Formats satoshis as BTC, without trailing zeros: 250000 is "0.0025".
pub fn format_btc(sats: u64) -> String {
    let frac = format!("{:08}", sats % SATS_PER_BTC);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        (sats / SATS_PER_BTC).to_string()
    } else {
        format!("{}.{}", sats / SATS_PER_BTC, frac)
    }
}
