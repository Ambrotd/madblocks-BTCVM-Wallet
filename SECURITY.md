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
| claim another network or chain | checks `/api/info` against what is pinned, and refuses the bridge |
| claim another signer set | pauses deposits and withdrawals and warns you, with where to check it (below) |
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
view the interface runs in. Each wallet in the app has its own key, vault and Windows Hello key, and its
own backup.

**Lookalike addresses.** Address poisoning pays you dust from an address that starts and ends like one you
use, hoping you copy it from your history next time. At review, a destination that shares its first six
and last four characters with a saved address or one of your wallets, without being it, is flagged in red.
Names in the address book are stripped of invisible and bidirectional characters, so a name can't disguise
the address next to it, and the book is checked again each time it's read.

## What it doesn't protect against

- **The peg's signers.** BTCVM's bridge is federated: m of n signers hold the locked BTC, and during the
  alpha one operator holds all the signer keys. The bridge's audit (`/api/status`) shows whether the peg is
  fully backed, but no wallet can stop the signers moving the locked BTC.
- **Malware running as you.** It can't decrypt the key without Windows Hello, but while a payment is being
  signed it could read the process's memory, and at any time it could change what the screen shows or swap
  an address you copied. Check the destination on the review screen against the one you were given, by a
  channel other than the clipboard when it matters.
- **BTCVM's consensus.** It runs on a single validator during the alpha.
- **Software that inspects HTTPS.** The app checks the bridge's certificate with Windows, as a browser
  does, so it works behind antivirus or company proxies that inspect TLS (Avast does by default). Such
  software can read the wallet's traffic with the bridge: addresses, balances, signed transactions. It
  never sees the key, and nothing it could change would move coins, since the wallet checks all of that
  itself.

## The trade-off: a change of signers pauses deposits and withdrawals

When BTCVM's operators rotate the signer set, the bridge starts reporting keys the wallet doesn't know, and
a new peg address. A hijacked server would look exactly the same. What tells them apart is the old signers
themselves: in a real rotation they move what they hold to the new set (`docs/ROTATION.md` and
`cmd/btcvm/rotate.go` in btc-vm), in transactions they sign, each tagged `BVMM` with the hash of the new
set's script. So, deliberately:

- **Deposits and withdrawals pause** as soon as the bridge reports another set. Sends on Bitcoin and on
  BTCVM keep working, and paying either peg directly stays refused.
- **The wallet looks for the old signers' move**, every ten minutes and when you ask: among the old peg's
  latest spends on BTCVM (from the bridge) and its tagged transactions on Bitcoin (from mempool.space). It
  follows a move only if the old set's m-of-n signatures on it verify, each SIGHASH_ALL, which commits them
  to the tag naming the new set; the coin's value comes from the transaction that made it, checked against
  its id. Neither server can forge that. Up to three rotations in a row are followed. Deposits and
  withdrawals then resume with the new set, and the wallet tells you, with the move's transaction.
- **Each move it followed is stored and checked again at every start**, from the set built into the
  wallet, so editing the settings file can't swap in another set; a proof that doesn't check out is
  ignored, and the wallet pauses again.
- **Until it finds the move, the wallet warns you.** It shows the old and new peg addresses and which keys
  changed, and links where to check them without relying on the bridge's server:
  - a Bitcoin explorer, where the BTC locked at the old peg should have moved to the new one, which only the
    old signers could have done;
  - BTCVM's rotation procedure (`docs/ROTATION.md` in btc-vm), and its docs and explorer at metalbtc.com,
    which are run by the same operators as the bridge;
  - madblocks, who publish each signer set they have verified with the wallet's updates.

The old signers are trusted to sign only real rotations: that is the bridge's own trust model, since they
can already move the locked BTC.

## Rules for the code

- The core has no `unsafe` code (`#![forbid(unsafe_code)]`) and does no networking: the app fetches, the
  core checks, plans and signs.
- Private keys live in buffers wiped on drop, print as `Key(…)`, and stay in Rust.
- Every change to transaction code must keep BTCVM's web wallet vectors passing byte for byte.
- Dependencies are few and pinned by the committed `Cargo.lock`, and CI runs `cargo audit`.
- `scripts/secretscan.sh` refuses WIFs, extended private keys and PEM private keys in any commit. It runs
  as the pre-commit and pre-push hooks and in CI. The test vectors' keys are public (SHA-256 of labels in
  BTCVM's, the keys BIPs 32, 39 and 84 publish in theirs) and are allowed by path.
- No telemetry. The app's requests go to the bridge you choose and, for what it doesn't serve, to
  mempool.space: Bitcoin fee estimates and BTC's price, every ten minutes, which say nothing about you (the
  price only if values are shown); an old transaction a pruned node no longer has (from blockstream.info
  if mempool.space doesn't answer), which tells that service the transaction's id; the old peg's latest
  transactions while a signer change waits to be checked; and, only if you turn it on in Settings, your
  Bitcoin balance, as a second opinion on the bridge's, which tells mempool.space your address. Every
  request carries a User-Agent naming the wallet and its version and nothing about you.
- A local log (`logs\wallet.log`, two files of at most 1 MB) records failures, payments sent and changes
  of the bridge's state, for support. It never holds a key or a recovery phrase.
