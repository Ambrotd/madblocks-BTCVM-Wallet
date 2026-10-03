//! Transactions: SegWit serialization (BIP144), parsing, ids, the BIP143
//! signature hash and size estimates, as the web wallet's chain.js does them.

use crate::encoding::sha256d;
use crate::{Result, invalid};

/// The version of the wallet's transactions.
pub(crate) const TX_VERSION: u32 = 2;
/// Inputs signal replaceability (BIP125), so a Bitcoin payment that stalls
/// can be sent again with a higher fee.
pub(crate) const SEQUENCE: u32 = 0xffff_fffd;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxOut {
    pub value: u64,
    pub script: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct TxIn {
    /// As displayed (big-endian).
    pub txid: [u8; 32],
    pub vout: u32,
    /// What the coin holds: SegWit signatures commit to it.
    pub value: u64,
    pub sequence: u32,
    pub witness: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub(crate) struct Tx {
    pub version: u32,
    pub inputs: Vec<TxIn>,
    pub outputs: Vec<TxOut>,
}

pub(crate) fn varint(n: usize, out: &mut Vec<u8>) {
    if n < 0xfd {
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(0xfd);
        out.extend_from_slice(&(n as u16).to_le_bytes());
    } else {
        out.push(0xfe);
        out.extend_from_slice(&(n as u32).to_le_bytes());
    }
}

fn varint_len(n: usize) -> usize {
    match n {
        0..0xfd => 1,
        0xfd..=0xffff => 3,
        _ => 5,
    }
}

fn outpoint(i: &TxIn, out: &mut Vec<u8>) {
    let mut id = i.txid;
    id.reverse();
    out.extend_from_slice(&id);
    out.extend_from_slice(&i.vout.to_le_bytes());
}

fn output(o: &TxOut, out: &mut Vec<u8>) {
    out.extend_from_slice(&o.value.to_le_bytes());
    varint(o.script.len(), out);
    out.extend_from_slice(&o.script);
}

impl Tx {
    /// The transaction's bytes, with its witnesses if it has any.
    pub fn serialize(&self) -> Vec<u8> {
        let witness = self.inputs.iter().any(|i| !i.witness.is_empty());
        let mut b = Vec::new();
        b.extend_from_slice(&self.version.to_le_bytes());
        if witness {
            b.extend_from_slice(&[0x00, 0x01]);
        }
        varint(self.inputs.len(), &mut b);
        for i in &self.inputs {
            outpoint(i, &mut b);
            varint(0, &mut b); // no scriptSig: P2WPKH signs in the witness
            b.extend_from_slice(&i.sequence.to_le_bytes());
        }
        varint(self.outputs.len(), &mut b);
        for o in &self.outputs {
            output(o, &mut b);
        }
        if witness {
            for i in &self.inputs {
                varint(i.witness.len(), &mut b);
                for item in &i.witness {
                    varint(item.len(), &mut b);
                    b.extend_from_slice(item);
                }
            }
        }
        b.extend_from_slice(&0u32.to_le_bytes());
        b
    }

    /// BIP143 SIGHASH_ALL for a P2WPKH input whose key hashes to `key_hash`.
    /// It commits to the amount the input spends.
    pub fn sighash(&self, index: usize, key_hash: &[u8]) -> [u8; 32] {
        let mut prevouts = Vec::new();
        let mut sequences = Vec::new();
        for i in &self.inputs {
            outpoint(i, &mut prevouts);
            sequences.extend_from_slice(&i.sequence.to_le_bytes());
        }
        let mut outputs = Vec::new();
        for o in &self.outputs {
            output(o, &mut outputs);
        }
        let input = &self.inputs[index];
        let mut pre = Vec::new();
        pre.extend_from_slice(&self.version.to_le_bytes());
        pre.extend_from_slice(&sha256d(&prevouts));
        pre.extend_from_slice(&sha256d(&sequences));
        outpoint(input, &mut pre);
        pre.extend_from_slice(&[0x19, 0x76, 0xa9, 0x14]);
        pre.extend_from_slice(key_hash);
        pre.extend_from_slice(&[0x88, 0xac]);
        pre.extend_from_slice(&input.value.to_le_bytes());
        pre.extend_from_slice(&input.sequence.to_le_bytes());
        pre.extend_from_slice(&sha256d(&outputs));
        pre.extend_from_slice(&0u32.to_le_bytes()); // lock time
        pre.extend_from_slice(&1u32.to_le_bytes()); // SIGHASH_ALL
        sha256d(&pre)
    }
}

/// What a transaction's bytes say: the coins it spends, as (txid, vout),
/// its outputs, and the bytes its id is the hash of.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub version: u32,
    pub inputs: Vec<(String, u32)>,
    /// Each input's sequence number: below 0xfffffffe, it signals that the
    /// transaction may be replaced (BIP 125).
    pub sequences: Vec<u32>,
    pub outputs: Vec<TxOut>,
    /// Each input's witness stack; empty for one without.
    pub witnesses: Vec<Vec<Vec<u8>>>,
    pub lock_time: u32,
    stripped: Vec<u8>,
}

impl Parsed {
    /// The transaction's id, as displayed: the double SHA-256 of it without
    /// witnesses, byte-reversed.
    pub fn txid(&self) -> String {
        let mut h = sha256d(&self.stripped);
        h.reverse();
        hex::encode(h)
    }
}

/// Reads a transaction, with or without witnesses.
pub fn parse_tx(raw: &[u8]) -> Result<Parsed> {
    let mut r = Reader { raw, at: 4 };
    r.need(2)?;
    let segwit = raw[4] == 0x00 && raw[5] == 0x01;
    if segwit {
        r.at += 2;
    }
    let body_start = r.at;
    let mut inputs = Vec::new();
    let mut sequences = Vec::new();
    for _ in 0..r.varint()? {
        let prev = r.take(36)?;
        let mut id = prev[..32].to_vec();
        id.reverse();
        let vout = u32::from_le_bytes(prev[32..].try_into().expect("4 bytes"));
        inputs.push((hex::encode(id), vout));
        let script = r.varint()?;
        r.take(script)?;
        sequences.push(u32::from_le_bytes(r.take(4)?.try_into().expect("4 bytes")));
    }
    let mut outputs = Vec::new();
    for _ in 0..r.varint()? {
        let value = u64::from_le_bytes(r.take(8)?.try_into().expect("8 bytes"));
        let script = r.varint()?;
        outputs.push(TxOut {
            value,
            script: r.take(script)?.to_vec(),
        });
    }
    let body_end = r.at;
    let mut witnesses = vec![Vec::new(); inputs.len()];
    if segwit {
        for witness in &mut witnesses {
            for _ in 0..r.varint()? {
                let item = r.varint()?;
                witness.push(r.take(item)?.to_vec());
            }
        }
    }
    r.need(4)?;
    if r.at + 4 != raw.len() {
        return invalid("trailing bytes after the transaction");
    }
    let lock_time = u32::from_le_bytes(raw[r.at..].try_into().expect("4 bytes"));
    let stripped = if segwit {
        [&raw[..4], &raw[body_start..body_end], &raw[r.at..]].concat()
    } else {
        raw.to_vec()
    };
    Ok(Parsed {
        version: u32::from_le_bytes(raw[..4].try_into().expect("4 bytes")),
        inputs,
        sequences,
        outputs,
        witnesses,
        lock_time,
        stripped,
    })
}

/// BIP143's SIGHASH_ALL digest for input `index` of the transaction `raw`,
/// spending `amount` under `script_code`: a P2WPKH input's `76a914…88ac`,
/// or a P2WSH input's witness script (without OP_CODESEPARATOR).
pub fn witness_sighash(
    raw: &[u8],
    index: usize,
    script_code: &[u8],
    amount: u64,
) -> Result<[u8; 32]> {
    let tx = parse_tx(raw)?;
    if index >= tx.inputs.len() {
        return invalid("no such input");
    }
    let outpoint = |(txid, vout): &(String, u32), out: &mut Vec<u8>| {
        let mut id = hex::decode(txid).expect("parsed from bytes");
        id.reverse();
        out.extend_from_slice(&id);
        out.extend_from_slice(&vout.to_le_bytes());
    };
    let mut prevouts = Vec::new();
    for i in &tx.inputs {
        outpoint(i, &mut prevouts);
    }
    let sequences: Vec<u8> = tx.sequences.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut outputs = Vec::new();
    for o in &tx.outputs {
        output(o, &mut outputs);
    }
    let mut pre = Vec::new();
    pre.extend_from_slice(&tx.version.to_le_bytes());
    pre.extend_from_slice(&sha256d(&prevouts));
    pre.extend_from_slice(&sha256d(&sequences));
    outpoint(&tx.inputs[index], &mut pre);
    varint(script_code.len(), &mut pre);
    pre.extend_from_slice(script_code);
    pre.extend_from_slice(&amount.to_le_bytes());
    pre.extend_from_slice(&tx.sequences[index].to_le_bytes());
    pre.extend_from_slice(&sha256d(&outputs));
    pre.extend_from_slice(&tx.lock_time.to_le_bytes());
    pre.extend_from_slice(&1u32.to_le_bytes()); // SIGHASH_ALL
    Ok(sha256d(&pre))
}

/// A transaction's id, from its bytes.
pub fn txid(raw: &[u8]) -> Result<String> {
    Ok(parse_tx(raw)?.txid())
}

struct Reader<'a> {
    raw: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn need(&self, n: usize) -> Result<()> {
        match self.at.checked_add(n) {
            Some(end) if end <= self.raw.len() => Ok(()),
            _ => invalid("truncated transaction"),
        }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        self.need(n)?;
        let s = &self.raw[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }

    fn varint(&mut self) -> Result<usize> {
        Ok(match self.take(1)?[0] {
            0xfd => u16::from_le_bytes(self.take(2)?.try_into().expect("2 bytes")) as usize,
            0xfe => u32::from_le_bytes(self.take(4)?.try_into().expect("4 bytes")) as usize,
            0xff => return invalid("transaction too large"),
            b => b as usize,
        })
    }
}

/// The virtual size of a transaction spending `inputs` P2WPKH coins to
/// `outputs`, once signed, taking signatures at their largest (73 bytes).
pub(crate) fn vsize(inputs: usize, outputs: &[TxOut]) -> u64 {
    let base = 4
        + 1
        + 41 * inputs
        + varint_len(outputs.len())
        + outputs
            .iter()
            .map(|o| 8 + varint_len(o.script.len()) + o.script.len())
            .sum::<usize>()
        + 4;
    let witness = 2 + inputs * (1 + 1 + 73 + 1 + 33);
    ((base * 4 + witness) as u64).div_ceil(4)
}
