//! The address book: names for addresses the user pays, kept as BTCVM's web
//! wallet keeps them (`cmd/btcvm/web/addressbook.js`).
//!
//! An entry names an address on one chain. A coin's own chain and its VM
//! share address formats (Bitcoin and BTCVM, Dogecoin and DogecoinVM), so the
//! chain is what the user said the address is for, not something read from
//! it: an exchange's Bitcoin deposit address paid on BTCVM never reaches the
//! exchange. Every address is checked with the core's decoder, for its
//! chain's coin, before it is stored, and a stored book is checked again when
//! it is read, so nothing malformed is ever trusted.

use crate::address::{Network, decode_address};
use crate::payment::Chain;
use crate::{Result, invalid};

pub const MAX_NAME: usize = 60;
pub const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Contact {
    pub name: String,
    /// The address's canonical text.
    pub address: String,
    pub chain: Chain,
}

/// A name as it is stored: control, zero-width and bidirectional-override
/// characters removed, so a name shown next to an address can't hide or
/// reorder text, and whitespace collapsed.
pub fn clean_name(name: &str) -> Result<String> {
    let hidden = |c: char| {
        matches!(c,
            '\u{0000}'..='\u{001f}' | '\u{007f}'..='\u{009f}' | '\u{00ad}' | '\u{061c}' | '\u{180e}'
            | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
    };
    let visible: String = name.chars().filter(|&c| !hidden(c)).collect();
    let clean = visible.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() {
        return invalid("give the address a name");
    }
    if clean.chars().count() > MAX_NAME {
        return invalid(format!("a name can be at most {MAX_NAME} characters"));
    }
    Ok(clean)
}

/// The canonical text of `address`, or why it isn't one.
pub fn canonical(address: &str, net: &Network) -> Result<String> {
    Ok(decode_address(address, net)?.address(net))
}

/// The book with a new entry, or why it can't be saved. The address must be
/// one of `chain`'s coin.
pub fn add(book: &[Contact], name: &str, address: &str, chain: Chain) -> Result<Vec<Contact>> {
    let name = clean_name(name)?;
    let address = canonical(address, &chain.coin().network())?;
    if let Some(dup) = book
        .iter()
        .find(|e| e.address == address && e.chain == chain)
    {
        return invalid(format!("already saved as \"{}\"", dup.name));
    }
    if book.len() >= MAX_ENTRIES {
        return invalid(format!(
            "the address book is full: it holds {MAX_ENTRIES} addresses; delete one first"
        ));
    }
    let mut next = book.to_vec();
    next.push(Contact {
        name,
        address,
        chain,
    });
    next.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.address.cmp(&b.address))
    });
    Ok(next)
}

pub fn rename(book: &[Contact], address: &str, chain: Chain, name: &str) -> Result<Vec<Contact>> {
    let name = clean_name(name)?;
    if !book
        .iter()
        .any(|e| e.address == address && e.chain == chain)
    {
        return invalid("that address is no longer in the book");
    }
    Ok(book
        .iter()
        .map(|e| {
            if e.address == address && e.chain == chain {
                Contact {
                    name: name.clone(),
                    ..e.clone()
                }
            } else {
                e.clone()
            }
        })
        .collect())
}

pub fn remove(book: &[Contact], address: &str, chain: Chain) -> Vec<Contact> {
    book.iter()
        .filter(|e| !(e.address == address && e.chain == chain))
        .cloned()
        .collect()
}

/// What the book says about `address` paid on `chain`: the entry saved for
/// that chain, or else one saved for the other chain (which the caller must
/// point out), or None.
pub fn find<'a>(book: &'a [Contact], address: &str, chain: Chain) -> Option<&'a Contact> {
    book.iter()
        .find(|e| e.address == address && e.chain == chain)
        .or_else(|| book.iter().find(|e| e.address == address))
}

/// Re-reads a stored book as if every entry were typed again: anything
/// malformed, invalid, duplicated or over the cap is dropped.
pub fn parse(stored: &[Contact]) -> Vec<Contact> {
    let mut book = Vec::new();
    for e in stored {
        if let Ok(next) = add(&book, &e.name, &e.address, e.chain) {
            book = next;
        }
    }
    book
}

/// Whether two different addresses look alike at a glance: the same first
/// six and last four characters. Address poisoning works this way: an
/// attacker pays dust from a lookalike of an address you use, hoping you copy
/// theirs from your history next time. BTCVM's web wallet asks for eight and
/// six; a GPU finds a match for those ends in days, and for these in seconds,
/// so these catch the cheap lookalikes too. Two real addresses share them by
/// chance about once in a billion.
pub fn lookalike(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
    a != b
        && a.len() > 14
        && b.len() > 14
        && a.get(..6) == b.get(..6)
        && a.get(a.len() - 4..) == b.get(b.len() - 4..)
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
    const B: &str = "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH";
    /// BIP 44's first Dogecoin address of the "abandon … about" phrase.
    const D: &str = "DBus3bamQjgJULBJtYXpEzDWQRwF5iwxgC";

    #[test]
    fn names_are_cleaned_and_bounded() {
        assert_eq!(
            clean_name("  My \u{202e}exchange\u{200b}  ").unwrap(),
            "My exchange"
        );
        assert!(clean_name("\u{200b}\u{200c} ").is_err());
        assert!(clean_name(&"x".repeat(61)).is_err());
        assert!(clean_name(&"x".repeat(60)).is_ok());
    }

    #[test]
    fn entries_are_checked_and_unique_per_chain() {
        let book = add(
            &[],
            "Exchange",
            " BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4 ",
            Chain::Bitcoin,
        )
        .unwrap();
        assert_eq!(book[0].address, A, "stored canonical");
        assert!(add(&book, "Again", A, Chain::Bitcoin).is_err());
        let book = add(&book, "Same key on BTCVM", A, Chain::Btcvm).unwrap();
        assert_eq!(book.len(), 2);
        assert!(
            add(
                &book,
                "Testnet",
                "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7",
                Chain::Bitcoin,
            )
            .is_err()
        );
        // An address is checked for its chain's coin: a Dogecoin one isn't
        // a Bitcoin one, nor the other way round.
        assert!(add(&book, "Doge", D, Chain::Bitcoin).is_err());
        assert!(add(&book, "Bitcoin", A, Chain::Dogecoin).is_err());
        let book = add(&book, "Doge", D, Chain::Dogecoinvm).unwrap();
        assert_eq!(book.len(), 3);
        let book = remove(&book, D, Chain::Dogecoinvm);

        let book = rename(&book, A, Chain::Btcvm, "Mine").unwrap();
        assert_eq!(find(&book, A, Chain::Btcvm).unwrap().name, "Mine");
        let book = remove(&book, A, Chain::Btcvm);
        // Saved for Bitcoin only: found, and the caller sees its chain.
        assert_eq!(find(&book, A, Chain::Btcvm).unwrap().chain, Chain::Bitcoin);
        assert!(find(&book, B, Chain::Bitcoin).is_none());
    }

    #[test]
    fn a_stored_book_is_checked_again() {
        let stored = vec![
            Contact {
                name: "Good".into(),
                address: A.into(),
                chain: Chain::Bitcoin,
            },
            Contact {
                name: "Duplicate".into(),
                address: A.into(),
                chain: Chain::Bitcoin,
            },
            Contact {
                name: "Bad".into(),
                address: "bc1qnotanaddress".into(),
                chain: Chain::Bitcoin,
            },
            Contact {
                name: "\u{200b}".into(),
                address: B.into(),
                chain: Chain::Bitcoin,
            },
            Contact {
                name: "Doge".into(),
                address: D.into(),
                chain: Chain::Dogecoin,
            },
            Contact {
                name: "Doge saved for Bitcoin".into(),
                address: D.into(),
                chain: Chain::Bitcoin,
            },
        ];
        let book = parse(&stored);
        assert_eq!(book.len(), 2);
        assert_eq!(book[0].name, "Doge");
        assert_eq!(book[1].name, "Good");
    }

    #[test]
    fn lookalikes_are_spotted() {
        let poisoned = "bc1qw508zzzzzzzzzzzzzzzzzzzzzzzzzzzz7kv8f3t4";
        assert!(lookalike(A, poisoned));
        // A cheap one: only the ends a glance takes in.
        assert!(lookalike(A, "bc1qw5zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzf3t4"));
        assert!(lookalike(A, &A.to_ascii_uppercase().replace("D6Q", "ZZZ")));
        assert!(!lookalike(A, A));
        assert!(!lookalike(A, &A.to_ascii_uppercase()), "the same address");
        assert!(!lookalike(A, B));
        assert!(!lookalike(
            A,
            "bc1qw5zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzf3t5"
        ));
        assert!(!lookalike(
            A,
            "bc1qw6zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzf3t4"
        ));
    }
}
