# Security

## Reporting a vulnerability

Report it privately with **Report a vulnerability** on this repository's Security tab, not in a public
issue. Say what you found, how to reproduce it, and what an attacker could do with it.

Problems in BTCVM itself (the chain, the bridge, its signers) belong to
[MetalBlockchain/btc-vm](https://github.com/MetalBlockchain/btc-vm).

## What the wallet protects against

**A dishonest or hijacked bridge server, or a tampered connection.** The wallet uses the bridge's API for
balances, coins and broadcasting, but nothing that moves coins rests on its word:

| The server could try to… | The wallet… |
| --- | --- |
| hand out its own deposit address | derives your deposit address from the signer set pinned in the wallet, and refuses a different one |
| point withdrawals at its own address | pays only the reserve made from the pinned signer set, and builds the tag naming your Bitcoin address itself |
| claim another signer set, network or chain | checks `/api/info` against what is pinned, and moves nothing between the chains if anything differs |
| inflate a coin's value to turn it into fee | reads each value from the transaction that created the coin, after checking those bytes hash to its id |
| push the fee up | caps it at 1,000 sat/vB and 250,000 sats a payment |
| get something else signed | shows each payment output by output, signs exactly that, and reads the signed bytes back to compare |

A server can still lie about balances and history, hold back coins or not broadcast a payment. It can
stall the wallet, but not redirect a payment. Paying the peg directly, without a tag, is refused too,
because the bridge couldn't tell whose coins they were.

**Someone else using your key** (in the Windows app, which is in progress). The key is made on your PC and
encrypted to a Windows Hello key held by the TPM. It is decrypted only to sign a payment, after Windows
Hello (PIN, fingerprint or face), and wiped from memory afterwards. The vault file is useless on another
PC. It lives in `%LOCALAPPDATA%`, which doesn't roam or sync to OneDrive. The key never reaches the web
view the interface runs in.

## What it doesn't protect against

- **The peg's signers.** BTCVM's bridge is federated: m of n signers hold the locked BTC, and during the
  alpha one operator holds all the signer keys. The bridge's audit (`/api/status`) shows whether the peg is
  fully backed, but no wallet can stop the signers moving the locked BTC.
- **Malware running as you.** It can't decrypt the key without Windows Hello, but while a payment is being
  signed it could read the process's memory, and at any time it could change what the screen shows or swap
  an address you copied. Check the destination on the review screen against the one you were given, by a
  channel other than the clipboard when it matters.
- **A rotation of the signer set.** When BTCVM's operators rotate it, the peg address changes and the wallet
  refuses to move coins between the chains until it is updated with the new set. Verifying a rotation from
  the old keys' signatures is planned.
- **BTCVM's consensus.** It runs on a single validator during the alpha.

## Rules for the code

- The core has no `unsafe` code (`#![forbid(unsafe_code)]`) and does no networking: the app fetches, the
  core checks, plans and signs.
- Private keys live in buffers wiped on drop, print as `Key(…)`, and stay in Rust.
- Every change to transaction code must keep BTCVM's web wallet vectors passing byte for byte.
- Dependencies are few and pinned by the committed `Cargo.lock`, and CI runs `cargo audit`.
- `scripts/secretscan.sh` refuses WIFs, extended private keys and PEM private keys in any commit. It runs
  as the pre-commit and pre-push hooks and in CI. The vectors' keys are SHA-256 of public labels and are
  allowed by path.
- No telemetry. The app's requests go only to the bridge you choose, with a User-Agent naming the wallet
  and its version and nothing about you.
