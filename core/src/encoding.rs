//! Hashes, base58check and bech32/bech32m (BIP173, BIP350).

use crate::{Error, Result, invalid};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub fn sha256d(data: &[u8]) -> [u8; 32] {
    Sha256::digest(Sha256::digest(data)).into()
}

pub fn hash160(data: &[u8]) -> [u8; 20] {
    Ripemd160::digest(Sha256::digest(data)).into()
}

// --- base58check -----------------------------------------------------------------

/// Encodes `payload` under a version byte. The payload may be a private key
/// (a WIF), so the buffer is wiped when dropped.
pub(crate) fn check_encode(version: u8, payload: &[u8]) -> String {
    let mut data = Zeroizing::new(Vec::with_capacity(1 + payload.len() + 4));
    data.push(version);
    data.extend_from_slice(payload);
    let sum = sha256d(&data);
    data.extend_from_slice(&sum[..4]);
    bs58::encode(&data[..]).into_string()
}

/// Decodes base58check into its version byte and payload, which is wiped
/// when dropped: it may be a private key.
pub(crate) fn check_decode(s: &str) -> Result<(u8, Zeroizing<Vec<u8>>)> {
    let raw = Zeroizing::new(
        bs58::decode(s.trim())
            .into_vec()
            .map_err(|_| Error::Invalid("not base58".into()))?,
    );
    if raw.len() < 5 {
        return invalid("too short");
    }
    let (data, sum) = raw.split_at(raw.len() - 4);
    if sha256d(data)[..4] != *sum {
        return invalid("checksum mismatch: check for a typo");
    }
    Ok((data[0], Zeroizing::new(data[1..].to_vec())))
}

// --- bech32 and bech32m ------------------------------------------------------------

pub(crate) const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32: u32 = 1;
const BECH32M: u32 = 0x2bc8_30a3;

fn polymod(values: &[u8]) -> u32 {
    const GEN: [u32; 5] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];
    let mut chk = 1u32;
    for &v in values {
        let top = chk >> 25;
        chk = ((chk & 0x01ff_ffff) << 5) ^ u32::from(v);
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

fn hrp_expand(hrp: &str) -> Vec<u8> {
    let mut v: Vec<u8> = hrp.bytes().map(|c| c >> 5).collect();
    v.push(0);
    v.extend(hrp.bytes().map(|c| c & 31));
    v
}

/// Regroups bits, as bech32 carries 5 bits per character.
fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Result<Vec<u8>> {
    let max = (1u32 << to) - 1;
    let max_acc = (1u32 << (from + to - 1)) - 1;
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut out = Vec::with_capacity(data.len() * from as usize / to as usize + 1);
    for &v in data {
        acc = ((acc << from) | u32::from(v)) & max_acc;
        bits += from;
        while bits >= to {
            bits -= to;
            out.push(((acc >> bits) & max) as u8);
        }
    }
    if pad {
        if bits > 0 {
            out.push(((acc << (to - bits)) & max) as u8);
        }
    } else if bits >= from || ((acc << (to - bits)) & max) != 0 {
        return invalid("invalid padding");
    }
    Ok(out)
}

/// Encodes a segwit address: bech32 for witness version 0, bech32m above.
pub fn segwit_encode(hrp: &str, version: u8, program: &[u8]) -> String {
    let mut data = vec![version];
    data.extend(convert_bits(program, 8, 5, true).expect("padding never fails"));
    let mut values = hrp_expand(hrp);
    values.extend_from_slice(&data);
    values.extend_from_slice(&[0; 6]);
    let m = polymod(&values) ^ if version == 0 { BECH32 } else { BECH32M };
    let mut s = String::with_capacity(hrp.len() + 1 + data.len() + 6);
    s.push_str(hrp);
    s.push('1');
    s.extend(data.iter().map(|&d| CHARSET[usize::from(d)] as char));
    s.extend((0..6).map(|i| CHARSET[((m >> (5 * (5 - i))) & 31) as usize] as char));
    s
}

/// Decodes a segwit address for `hrp` into its witness version and program.
/// The checksum must be the one its version calls for (BIP350), and a
/// version 0 program 20 or 32 bytes (BIP141).
pub fn segwit_decode(address: &str, hrp: &str) -> Result<(u8, Vec<u8>)> {
    let lower = address.to_ascii_lowercase();
    if address != lower && address != address.to_ascii_uppercase() {
        return invalid("mixed-case address");
    }
    let s = lower;
    let pos = match s.rfind('1') {
        Some(p) if p >= 1 && p + 7 <= s.len() && s.len() <= 90 => p,
        _ => return invalid("not a valid address"),
    };
    if &s[..pos] != hrp {
        return invalid("address is for a different network");
    }
    let data = s[pos + 1..]
        .bytes()
        .map(|c| CHARSET.iter().position(|&x| x == c).map(|p| p as u8))
        .collect::<Option<Vec<u8>>>()
        .ok_or_else(|| Error::Invalid("invalid character in address".into()))?;
    // A version, then the program, then six characters of checksum.
    if data.len() < 7 || data[0] > 16 {
        return invalid("not a valid address");
    }
    let version = data[0];
    let mut values = hrp_expand(hrp);
    values.extend_from_slice(&data);
    if polymod(&values) != if version == 0 { BECH32 } else { BECH32M } {
        return invalid("checksum mismatch: check for a typo");
    }
    let program = convert_bits(&data[1..data.len() - 6], 5, 8, false)?;
    if program.len() < 2
        || program.len() > 40
        || (version == 0 && program.len() != 20 && program.len() != 32)
    {
        return invalid("not a valid address");
    }
    Ok((version, program))
}
