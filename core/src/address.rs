//! Addresses and output scripts. Bitcoin and BTCVM share every format, so a
//! key or script has the same address on both.

use crate::encoding::{CHARSET, check_decode, check_encode, segwit_decode, segwit_encode};
use crate::{Result, invalid};

/// A network's address and key encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network {
    pub p2pkh: u8,
    pub p2sh: u8,
    pub wif: u8,
    pub hrp: &'static str,
}

/// Bitcoin mainnet's encodings, which BTCVM mainnet shares.
pub const MAINNET: Network = Network {
    p2pkh: 0,
    p2sh: 5,
    wif: 128,
    hrp: "bc",
};

/// Bitcoin testnet's, which BTCVM testnet shares.
pub const TESTNET: Network = Network {
    p2pkh: 111,
    p2sh: 196,
    wif: 239,
    hrp: "tb",
};

/// Destination kinds, numbered as in btc-vm's `tags.go`: the number travels
/// in peg tags (BVMO, BVMD) and in deposit scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// `1…`
    P2pkh = 0,
    /// `3…`
    P2sh = 1,
    /// `bc1q…`, a 20-byte key hash
    P2wpkh = 2,
    /// `bc1q…`, a 32-byte script hash
    P2wsh = 3,
    /// `bc1p…`
    P2tr = 4,
}

impl Kind {
    pub fn from_byte(b: u8) -> Option<Kind> {
        Some(match b {
            0 => Kind::P2pkh,
            1 => Kind::P2sh,
            2 => Kind::P2wpkh,
            3 => Kind::P2wsh,
            4 => Kind::P2tr,
            _ => return None,
        })
    }

    /// The length of the kind's program: its hash, or for Taproot its key.
    pub fn program_len(self) -> usize {
        match self {
            Kind::P2pkh | Kind::P2sh | Kind::P2wpkh => 20,
            Kind::P2wsh | Kind::P2tr => 32,
        }
    }
}

/// Where coins go: a kind and its program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Destination {
    kind: Kind,
    program: Vec<u8>,
}

impl Destination {
    pub fn new(kind: Kind, program: &[u8]) -> Result<Destination> {
        if program.len() != kind.program_len() {
            return invalid("malformed destination");
        }
        Ok(Destination {
            kind,
            program: program.to_vec(),
        })
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn program(&self) -> &[u8] {
        &self.program
    }

    pub fn pk_script(&self) -> Vec<u8> {
        let p = &self.program[..];
        match self.kind {
            Kind::P2pkh => [&[0x76, 0xa9, 0x14][..], p, &[0x88, 0xac]].concat(),
            Kind::P2sh => [&[0xa9, 0x14][..], p, &[0x87]].concat(),
            Kind::P2wpkh | Kind::P2wsh => [&[0x00, p.len() as u8][..], p].concat(),
            Kind::P2tr => [&[0x51, 0x20][..], p].concat(),
        }
    }

    pub fn address(&self, net: &Network) -> String {
        match self.kind {
            Kind::P2pkh => check_encode(net.p2pkh, &self.program),
            Kind::P2sh => check_encode(net.p2sh, &self.program),
            Kind::P2wpkh | Kind::P2wsh => segwit_encode(net.hrp, 0, &self.program),
            Kind::P2tr => segwit_encode(net.hrp, 1, &self.program),
        }
    }

    /// Reads a standard output script, or returns None for anything else.
    pub fn from_script(s: &[u8]) -> Option<Destination> {
        let (kind, program) = match s {
            [0x76, 0xa9, 0x14, h @ .., 0x88, 0xac] if h.len() == 20 => (Kind::P2pkh, h),
            [0xa9, 0x14, h @ .., 0x87] if h.len() == 20 => (Kind::P2sh, h),
            [0x00, 0x14, h @ ..] if h.len() == 20 => (Kind::P2wpkh, h),
            [0x00, 0x20, h @ ..] if h.len() == 32 => (Kind::P2wsh, h),
            [0x51, 0x20, h @ ..] if h.len() == 32 => (Kind::P2tr, h),
            _ => return None,
        };
        Some(Destination {
            kind,
            program: program.to_vec(),
        })
    }

    /// The kind and program together, as peg tags and deposit scripts carry
    /// a destination.
    pub fn tag_bytes(&self) -> Vec<u8> {
        [&[self.kind as u8][..], &self.program].concat()
    }

    pub fn from_tag_bytes(b: &[u8]) -> Option<Destination> {
        let (&kind, program) = b.split_first()?;
        Destination::new(Kind::from_byte(kind)?, program).ok()
    }
}

/// Decodes an address for the network `net`: base58 (`1…`, `3…`) or
/// segwit (`bc1q…`, `bc1p…`). Other witness versions are refused: nothing
/// can spend them yet, so paying one would burn the coins.
pub fn decode_address(address: &str, net: &Network) -> Result<Destination> {
    let address = address.trim();
    if address
        .to_ascii_lowercase()
        .starts_with(&format!("{}1", net.hrp))
    {
        let (version, program) = segwit_decode(address, net.hrp)?;
        return match (version, program.len()) {
            (0, 20) => Destination::new(Kind::P2wpkh, &program),
            (0, 32) => Destination::new(Kind::P2wsh, &program),
            (1, 32) => Destination::new(Kind::P2tr, &program),
            _ => invalid("unsupported SegWit address"),
        };
    }
    if looks_like_bech32(address) {
        return invalid("address is for a different network");
    }
    let (version, payload) = check_decode(address)?;
    if payload.len() != 20 {
        return invalid("not an address");
    }
    if version == net.p2pkh {
        Destination::new(Kind::P2pkh, &payload)
    } else if version == net.p2sh {
        Destination::new(Kind::P2sh, &payload)
    } else {
        invalid("address is for a different network")
    }
}

/// Whether `s` has the shape of a bech32 string (letters, `1`, data), so a
/// `tb1…` or `ltc1…` address gets a clear error instead of a base58 one.
fn looks_like_bech32(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let Some(pos) = lower.find('1') else {
        return false;
    };
    let (hrp, data) = (&lower[..pos], &lower[pos + 1..]);
    (1..=83).contains(&hrp.len())
        && hrp.bytes().all(|c| c.is_ascii_lowercase())
        && data.len() >= 6
        && data.bytes().all(|c| CHARSET.contains(&c))
}

/// A script push of `data`, in its minimal form for up to 255 bytes.
pub(crate) fn push_data(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 0x4c {
        Ok([&[data.len() as u8][..], data].concat())
    } else if data.len() <= 0xff {
        Ok([&[0x4c, data.len() as u8][..], data].concat())
    } else {
        invalid("push too large")
    }
}
