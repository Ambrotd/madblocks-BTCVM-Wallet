//! Signs madblocks BTCVM Wallet releases (see RELEASING.md).
//!
//!     release-tool keygen <dir>
//!         Makes madblocks' update key: <dir>/madblocks-update.key, encrypted
//!         with a password (MADBLOCKS_UPDATE_PASSWORD, or asked for), and
//!         <dir>/madblocks-update.pub. The
//!         public key goes into app/src/update.rs; the secret key never goes
//!         into the repository.
//!
//!     release-tool manifest <exe> <version> <url> [<notes.json>]
//!         Prints latest.json for <exe>, to be published at <url>: its
//!         version, SHA-256 and size, and notes ({"es": …, "en": …}).
//!
//!     release-tool sign <secret key> <file>…
//!         Writes <file>.minisig for each file. The password comes from
//!         MADBLOCKS_UPDATE_PASSWORD, or is asked for.
//!
//!     release-tool verify <public key> <file>…
//!         Checks each <file>.minisig; the key as a file or as its base64.

use minisign::{KeyPair, PublicKey, SecretKey, SignatureBox};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["keygen", dir] => keygen(Path::new(dir)),
        ["manifest", exe, version, url] => manifest(Path::new(exe), version, url, None),
        ["manifest", exe, version, url, notes] => manifest(Path::new(exe), version, url, Some(Path::new(notes))),
        ["sign", key, files @ ..] if !files.is_empty() => sign(Path::new(key), files),
        ["verify", key, files @ ..] if !files.is_empty() => verify(key, files),
        _ => Err("usage: release-tool keygen <dir> | manifest <exe> <version> <url> [notes.json] | sign <secret key> <file>… | verify <public key> <file>…".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("release-tool: {e}");
            ExitCode::FAILURE
        }
    }
}

fn keygen(dir: &Path) -> Result<(), String> {
    let secret = dir.join("madblocks-update.key");
    let public = dir.join("madblocks-update.pub");
    if secret.exists() || public.exists() {
        return Err(format!(
            "{} already has an update key; not replacing it",
            dir.display()
        ));
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // The password from MADBLOCKS_UPDATE_PASSWORD, or asked for twice.
    let password = std::env::var("MADBLOCKS_UPDATE_PASSWORD").ok();
    let KeyPair { pk, sk } =
        KeyPair::generate_encrypted_keypair(password).map_err(|e| e.to_string())?;
    let sk_box = sk
        .to_box(Some(
            "madblocks BTCVM Wallet update key (secret): keep it off the repository",
        ))
        .map_err(|e| e.to_string())?;
    fs::write(&secret, sk_box.into_string()).map_err(|e| e.to_string())?;
    fs::write(
        &public,
        pk.to_box().map_err(|e| e.to_string())?.into_string(),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "secret key: {} (back it up; anyone with it and its password can sign updates)",
        secret.display()
    );
    println!("public key: {}", public.display());
    println!("for app/src/update.rs, UPDATE_KEY:\n{}", pk.to_base64());
    Ok(())
}

fn manifest(exe: &Path, version: &str, url: &str, notes: Option<&Path>) -> Result<(), String> {
    let bytes = fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    if !url.starts_with("https://") {
        return Err("the download URL must be https://".into());
    }
    let notes: serde_json::Value = match notes {
        Some(path) => serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("{}: {e}", path.display()))?,
        None => serde_json::json!({}),
    };
    let manifest = serde_json::json!({
        "version": version.trim_start_matches('v'),
        "url": url,
        "sha256": hex::encode(Sha256::digest(&bytes)),
        "size": bytes.len(),
        "notes": notes,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn sign(key: &Path, files: &[&str]) -> Result<(), String> {
    let password = std::env::var("MADBLOCKS_UPDATE_PASSWORD").ok();
    let sk = SecretKey::from_file(key, password).map_err(|e| e.to_string())?;
    for file in files {
        let data = fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        let name = Path::new(file)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file);
        let comment = format!("madblocks BTCVM Wallet release: {name}");
        let sig = minisign::sign(None, &sk, Cursor::new(&data), Some(&comment), None)
            .map_err(|e| e.to_string())?;
        let out = PathBuf::from(format!("{file}.minisig"));
        fs::write(&out, sig.into_string()).map_err(|e| e.to_string())?;
        println!("signed {file} -> {}", out.display());
    }
    Ok(())
}

fn verify(key: &str, files: &[&str]) -> Result<(), String> {
    let pk = if Path::new(key).exists() {
        PublicKey::from_file(key)
    } else {
        PublicKey::from_base64(key)
    }
    .map_err(|e| e.to_string())?;
    for file in files {
        let data = fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        let sig = SignatureBox::from_file(format!("{file}.minisig")).map_err(|e| e.to_string())?;
        minisign::verify(&pk, &sig, Cursor::new(&data), true, false, false)
            .map_err(|e| format!("{file}: {e}"))?;
        println!("{file}: good signature");
    }
    Ok(())
}
