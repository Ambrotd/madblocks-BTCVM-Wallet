# Releasing

madblocks BTCVM Wallet ships as one standalone exe: no installer. The exe carries the window's files, keeps
its data in `%LOCALAPPDATA%\madblocks BTCVM Wallet` (the vaults, which only open on this PC with its
Windows Hello, the settings and the log), and needs Microsoft Edge WebView2 Runtime, which Windows 11 has and
Windows 10 usually has; without it the app says so and links the download. Deleting the exe deletes no
wallet: "Remove" in the app does, after Windows Hello.

Each release is published with a manifest, `latest.json`, signed with madblocks' update key. The app checks
that signature against the key built into it, takes only a newer version, checks the exe's size and SHA-256
against the signed manifest, and replaces itself only when the user says so (see `app/src/update.rs`).

## Once: the update key

```sh
cargo run -p release-tool -- keygen <a folder outside the repository>
```

It asks for a password and writes `madblocks-update.key` (the secret key, encrypted with that password) and
`madblocks-update.pub`. Back up both the key file and the password, apart from each other: with them anyone
can sign updates that every wallet installs; without them, no wallet can be updated except by hand.

Then:

1. Put the public key it prints in `app/src/update.rs` as `UPDATE_KEY`, and the releases' address as
   `RELEASES`: `https://github.com/<owner>/<repo>/releases/latest/download`. A build without them doesn't
   update itself.
2. In the GitHub repository's Settings > Secrets and variables > Actions, add `UPDATE_SECRET_KEY` (the
   contents of `madblocks-update.key`) and `UPDATE_KEY_PASSWORD`.

## Optional: an Authenticode signature

Windows SmartScreen warns about a downloaded exe without a known publisher signature ("Windows protected your
PC") until the file earns a reputation. An Authenticode signature makes the publisher visible and earns that
reputation. With a code-signing certificate (OV or EV, as a `.pfx`), add the secrets `WINDOWS_CERTIFICATE`
(the `.pfx`, base64: `[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))`) and
`WINDOWS_CERTIFICATE_PASSWORD`; the release workflow then signs the exe with `signtool` before the minisign
signatures. Azure Trusted Signing works too, with its own action in place of that step.

## Each release

1. Raise the version in `app/Cargo.toml` and `app/tauri.conf.json` (the workflow refuses a tag that
   disagrees).
2. Write `release-notes/<version>.md` (the GitHub release's page) and `release-notes/<version>.json`, the
   notes the app shows: `{"es": "…", "en": "…"}`.
3. Commit, tag `v<version>` and push the tag. The `release` workflow tests everything, builds the exe, signs
   it, writes `latest.json` and signs it, and publishes the release with the exe, its `.minisig` and
   `.sha256`, and `latest.json` with its `.minisig`.
4. Wallets with the update key find it within twelve hours, or at once from Settings.

To sign by hand instead: `cargo build --release -p madblocks-btcvm-wallet`, then
`release-tool manifest <exe> <version> <url> [notes.json] > latest.json` and
`release-tool sign <secret key> latest.json <exe>`.

## Checking a download

The exe's signature, with the public key (`madblocks-update.pub`, or the `UPDATE_KEY` in `app/src/update.rs`):

```sh
minisign -Vm madblocks-btcvm-wallet-<version>-x64.exe -P <public key>
```

or `cargo run -p release-tool -- verify <public key> <exe>`. Its SHA-256 is in the `.sha256` file beside it:
`Get-FileHash <exe>` in PowerShell.

## When BTCVM's signers rotate

Wallets follow a rotation by themselves once they find the old signers' signed move of the funds to the new
set (`core/src/rotation.rs`). A release with the new set built in (`core/src/bridge.rs`, `mod mainnet`, and
its fixture test) still matters: new installs start from it, and a wallet that missed several rotations
doesn't have to follow them all.
