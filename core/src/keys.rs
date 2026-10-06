//! Private keys. The wallet's own address is native SegWit (P2WPKH,
//! `bc1q…`), the same on Bitcoin and BTCVM; on Dogecoin and DogecoinVM,
//! which have no SegWit, it is P2PKH (`D…`).

use crate::address::{Destination, Kind, Network};
use crate::encoding::{check_decode, check_encode, hash160};
use crate::payment::Coin;
use crate::{Result, invalid};
use k256::ecdsa::SigningKey;
use zeroize::{Zeroize, Zeroizing};

/// A private key, wiped from memory when dropped.
pub struct Key(Zeroizing<[u8; 32]>);

/// Never prints the key, so it can't end up in a log by accident.
impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(…)")
    }
}

impl Key {
    pub fn from_bytes(b: &[u8]) -> Result<Key> {
        if b.len() != 32 {
            return invalid("a private key is 32 bytes");
        }
        if SigningKey::from_slice(b).is_err() {
            return invalid("not a valid private key");
        }
        let mut k = Zeroizing::new([0u8; 32]);
        k.copy_from_slice(b);
        Ok(Key(k))
    }

    /// A new random key, from the operating system's generator.
    pub fn generate() -> Key {
        let mut bytes = SigningKey::random(&mut rand_core::OsRng).to_bytes();
        let key = Key::from_bytes(&bytes).expect("a fresh key is valid");
        bytes[..].zeroize();
        key
    }

    /// Parses a key typed or pasted by the user: 64 hex characters, or a
    /// compressed-key WIF (`K…` or `L…` on mainnet; any network's is taken,
    /// the key being the same). An uncompressed-key WIF would open a
    /// different, empty wallet, so it is refused.
    pub fn parse(s: &str) -> Result<Key> {
        let s = s.trim();
        if s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Key::from_bytes(&Zeroizing::new(hex::decode(s).expect("checked hex")));
        }
        let (_, payload) = check_decode(s)?;
        match payload.len() {
            32 => invalid("this is an uncompressed-key WIF; export a compressed one"),
            33 if payload[32] == 1 => Key::from_bytes(&payload[..32]),
            _ => invalid("not a private key"),
        }
    }

    /// The key's bytes, for the vault to encrypt. Nothing else needs them.
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) fn signing_key(&self) -> SigningKey {
        SigningKey::from_slice(&self.0[..]).expect("checked when made")
    }

    /// The compressed public key.
    pub fn public_key(&self) -> [u8; 33] {
        let point = self.signing_key().verifying_key().to_encoded_point(true);
        let mut out = [0u8; 33];
        out.copy_from_slice(point.as_bytes());
        out
    }

    /// The wallet's address: P2WPKH of the compressed public key.
    pub fn destination(&self) -> Destination {
        Destination::new(Kind::P2wpkh, &hash160(&self.public_key())).expect("20 bytes")
    }

    /// P2PKH of the compressed public key: the wallet's address on Dogecoin
    /// and DogecoinVM.
    pub fn p2pkh_destination(&self) -> Destination {
        Destination::new(Kind::P2pkh, &hash160(&self.public_key())).expect("20 bytes")
    }

    /// The wallet's address for `coin`'s chains.
    pub fn destination_for(&self, coin: Coin) -> Destination {
        match coin {
            Coin::Btc => self.destination(),
            Coin::Doge => self.p2pkh_destination(),
        }
    }

    /// The key as a compressed-key WIF, for backup.
    pub fn wif(&self, net: &Network) -> Zeroizing<String> {
        let mut payload = Zeroizing::new([0u8; 33]);
        payload[..32].copy_from_slice(&self.0[..]);
        payload[32] = 1;
        Zeroizing::new(check_encode(net.wif, &payload[..]))
    }
}
