//! Amounts: satoshis inside, BTC strings for people and for the bridge's
//! API. DOGE has the same eight decimals (koinu), with more whole digits.

use crate::{Result, invalid};

pub const SATS_PER_BTC: u64 = 100_000_000;
/// Bitcoin's whole supply: the most a single output can hold on either
/// chain.
pub const MAX_MONEY: u64 = 21_000_000 * SATS_PER_BTC;

/// Parses a BTC amount like "0.0025", with up to 8 decimals, into satoshis.
pub fn parse_btc(s: &str) -> Result<u64> {
    parse_units(s, 8, MAX_MONEY, "0.0025", "more than 21 million BTC")
}

/// Formats satoshis as BTC, without trailing zeros: 250000 is "0.0025".
pub fn format_btc(sats: u64) -> String {
    format_units(sats)
}

/// Parses an amount with up to `whole_digits` whole digits and 8 decimals
/// into its smallest units, refusing more than `max`.
pub(crate) fn parse_units(
    s: &str,
    whole_digits: usize,
    max: u64,
    example: &str,
    too_much: &str,
) -> Result<u64> {
    let s = s.trim();
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    let digits = |t: &str, most: usize| {
        (1..=most).contains(&t.len()) && t.bytes().all(|c| c.is_ascii_digit())
    };
    if !digits(whole, whole_digits) || frac.is_some_and(|f| !digits(f, 8)) {
        return invalid(format!("enter an amount like {example}"));
    }
    let whole: u64 = whole.parse().expect("checked digits");
    let frac: u64 = frac.map_or(0, |f| format!("{f:0<8}").parse().expect("checked digits"));
    match whole
        .checked_mul(SATS_PER_BTC)
        .and_then(|w| w.checked_add(frac))
    {
        Some(units) if units <= max => Ok(units),
        _ => invalid(too_much),
    }
}

/// Formats an amount of 8 decimals without trailing zeros: 250000 is
/// "0.0025".
pub(crate) fn format_units(units: u64) -> String {
    let frac = format!("{:08}", units % SATS_PER_BTC);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        (units / SATS_PER_BTC).to_string()
    } else {
        format!("{}.{}", units / SATS_PER_BTC, frac)
    }
}
