//! Updates, for the standalone exe.
//!
//! madblocks publishes each release with a manifest, `latest.json`, signed
//! with madblocks' update key (minisign; see RELEASING.md). The app checks
//! that signature against the key built into it, takes only a newer
//! version, downloads the exe the manifest names and checks its size and
//! SHA-256, and replaces itself only when the user says so. A server that
//! serves anything else, an old release included, is refused.

use crate::journal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

/// madblocks' update key: the public key `release-tool keygen` prints.
/// Empty in a build that doesn't update itself.
pub const UPDATE_KEY: &str = "RWRPELLfm+vTRf5eFKLQZdEckDGcPgPOgzrxuH54kCRTuEYSo7a9wqU4";

/// Where releases are published, with `latest.json` and its signature: the
/// latest release of the wallet's repository (`about::SOURCE_URL`). Empty in
/// a build that doesn't update itself.
pub const RELEASES: &str =
    "https://github.com/Ambrotd/madblocks-Metal-Wallet/releases/latest/download";

/// No exe this wallet ships comes near it.
const MAX_EXE: u64 = 64 << 20;

/// What a release says about itself, signed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Manifest {
    pub version: String,
    /// The exe, https.
    pub url: String,
    /// Of the exe, hex.
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub notes: Notes,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Notes {
    #[serde(default)]
    pub es: String,
    #[serde(default)]
    pub en: String,
}

/// Whether this build updates itself.
pub fn configured() -> bool {
    !UPDATE_KEY.is_empty() && RELEASES.starts_with("https://")
}

/// Checks `manifest` against its minisign `signature` by `key`; it must be
/// for a version newer than `current` to count.
pub fn check_manifest(
    manifest: &[u8],
    signature: &str,
    key: &str,
    current: &str,
) -> Result<Option<Manifest>, String> {
    let key = minisign_verify::PublicKey::from_base64(key)
        .map_err(|e| format!("the update key in this build is invalid: {e}"))?;
    let signature = minisign_verify::Signature::decode(signature)
        .map_err(|_| "the update's signature isn't readable".to_string())?;
    key.verify(manifest, &signature, false)
        .map_err(|_| "the update isn't signed by madblocks' key, so it is ignored".to_string())?;
    let m: Manifest = serde_json::from_slice(manifest)
        .map_err(|e| format!("the update's manifest isn't readable: {e}"))?;
    if !m.url.starts_with("https://") || m.sha256.len() != 64 || m.size == 0 || m.size > MAX_EXE {
        return Err("the update's manifest is malformed".into());
    }
    Ok(newer(&m.version, current).then_some(m))
}

/// Whether version `a` (x.y.z) is newer than `b`.
pub fn newer(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Option<(u64, u64, u64)> {
        let mut parts = v.trim().trim_start_matches('v').split('.');
        let mut next = || parts.next()?.parse::<u64>().ok();
        let version = (next()?, next()?, next()?);
        parts.next().is_none().then_some(version)
    };
    matches!((parse(a), parse(b)), (Some(x), Some(y)) if x > y)
}

fn agent(seconds: u64) -> ureq::Agent {
    crate::api::agent(Some(Duration::from_secs(seconds)))
}

fn fetch(url: &str, limit: u64, seconds: u64) -> Result<Vec<u8>, String> {
    let mut response = agent(seconds)
        .get(url)
        .call()
        .map_err(|e| format!("can't reach the releases: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("the releases answered {}", response.status()));
    }
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|e| format!("the download failed: {e}"))
}

/// The newest release, if it is newer than this one and properly signed.
pub fn latest() -> Result<Option<Manifest>, String> {
    if !configured() {
        return Ok(None);
    }
    let manifest = fetch(&format!("{RELEASES}/latest.json"), 1 << 16, 30)?;
    let signature = fetch(&format!("{RELEASES}/latest.json.minisig"), 1 << 12, 30)?;
    let signature = String::from_utf8(signature)
        .map_err(|_| "the update's signature isn't text".to_string())?;
    check_manifest(&manifest, &signature, UPDATE_KEY, env!("CARGO_PKG_VERSION"))
}

/// Downloads the exe `m` names, checks it against `m`, and puts it in place
/// of the running one, which stays beside it (`.old`) until the new one
/// starts. Returns the exe to start.
pub fn install(m: &Manifest) -> Result<PathBuf, String> {
    let bytes = fetch(&m.url, MAX_EXE, 600)?;
    put_in_place(
        m,
        &bytes,
        &std::env::current_exe().map_err(|e| e.to_string())?,
    )
}

fn put_in_place(m: &Manifest, bytes: &[u8], exe: &std::path::Path) -> Result<PathBuf, String> {
    if bytes.len() as u64 != m.size
        || hex::encode(Sha256::digest(bytes)) != m.sha256.to_ascii_lowercase()
    {
        return Err(
            "the download doesn't match the signed manifest, so nothing was changed".into(),
        );
    }
    let new = exe.with_extension("new");
    let old = exe.with_extension("old");
    let write = || -> std::io::Result<()> {
        let mut f = fs::File::create(&new)?;
        f.write_all(bytes)?;
        f.sync_all()
    };
    write().map_err(|e| format!("can't write the new version next to this one ({e}); download it from the releases page instead"))?;
    let _ = fs::remove_file(&old);
    // A running exe can be renamed on Windows, though not overwritten.
    fs::rename(exe, &old).map_err(|e| format!("can't set this version aside: {e}"))?;
    if let Err(e) = fs::rename(&new, exe) {
        let _ = fs::rename(&old, exe);
        return Err(format!("can't put the new version in place: {e}"));
    }
    journal::info(&format!("installed version {} from {}", m.version, m.url));
    Ok(exe.to_path_buf())
}

/// Removes what an update left: the previous exe, or a download not put in
/// place.
pub fn clean_up() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = fs::remove_file(exe.with_extension("old"));
        let _ = fs::remove_file(exe.with_extension("new"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minisign::KeyPair;
    use std::io::Cursor;

    fn signed(manifest: &[u8], keys: &KeyPair) -> String {
        minisign::sign(None, &keys.sk, Cursor::new(manifest), None, None)
            .unwrap()
            .into_string()
    }

    fn manifest(version: &str, exe: &[u8]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "version": version,
            "url": "https://example.org/wallet.exe",
            "sha256": hex::encode(Sha256::digest(exe)),
            "size": exe.len(),
            "notes": { "es": "Mejoras", "en": "Improvements" },
        }))
        .unwrap()
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("0.10.0", "0.9.9"));
        assert!(newer("v1.0.0", "0.99.99"));
        assert!(!newer("0.2.0", "0.2.0"));
        assert!(!newer("0.1.9", "0.2.0"));
        assert!(!newer("0.3.0-beta", "0.2.0"));
        assert!(!newer("0.3", "0.2.0"));
    }

    #[test]
    fn only_a_newer_release_signed_by_the_key_counts() {
        let keys = KeyPair::generate_unencrypted_keypair().unwrap();
        let key = keys.pk.to_base64();
        let exe = b"the new exe";
        let m = manifest("0.9.0", exe);
        let found = check_manifest(&m, &signed(&m, &keys), &key, "0.2.0")
            .unwrap()
            .unwrap();
        assert_eq!(
            (found.version.as_str(), found.notes.es.as_str()),
            ("0.9.0", "Mejoras")
        );
        // Not newer: nothing to do, even signed.
        let old = manifest("0.1.0", exe);
        assert_eq!(
            check_manifest(&old, &signed(&old, &keys), &key, "0.2.0").unwrap(),
            None
        );
        // Signed by another key, or changed after signing: refused.
        let other = KeyPair::generate_unencrypted_keypair().unwrap();
        assert!(check_manifest(&m, &signed(&m, &other), &key, "0.2.0").is_err());
        let mut changed = m.clone();
        changed[10] ^= 1;
        assert!(check_manifest(&changed, &signed(&m, &keys), &key, "0.2.0").is_err());
        assert!(check_manifest(&m, "not a signature", &key, "0.2.0").is_err());
    }

    #[test]
    fn the_new_exe_replaces_the_old_only_if_it_is_the_one_signed() {
        let dir = std::env::temp_dir().join(format!("btcvm-update-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("wallet.exe");
        fs::write(&exe, b"old").unwrap();
        let m: Manifest = serde_json::from_slice(&manifest("9.0.0", b"new")).unwrap();
        assert!(put_in_place(&m, b"evil", &exe).is_err());
        assert_eq!(fs::read(&exe).unwrap(), b"old");
        put_in_place(&m, b"new", &exe).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"new");
        assert_eq!(fs::read(exe.with_extension("old")).unwrap(), b"old");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn this_build_updates_itself_only_with_a_key_and_a_place() {
        assert_eq!(
            configured(),
            !UPDATE_KEY.is_empty() && RELEASES.starts_with("https://")
        );
    }

    #[test]
    fn the_update_key_is_a_minisign_public_key() {
        // A mistyped key would leave every wallet unable to update.
        assert!(
            UPDATE_KEY.is_empty() || minisign_verify::PublicKey::from_base64(UPDATE_KEY).is_ok()
        );
    }

    #[test]
    fn releases_come_from_the_wallets_repository() {
        assert!(RELEASES.is_empty() || RELEASES.starts_with(btcvm_wallet_core::about::SOURCE_URL));
    }
}
