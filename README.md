<h1 align="center">madblocks BTCVM Wallet</h1>

<p align="center">
  A Windows wallet for <strong>Bitcoin</strong> and <strong>BTCVM</strong>, and the bridge between them.<br/>
  One key, one address on both networks, and Windows Hello for every payment.
</p>

<p align="center">
  Created by <a href="https://madblocks.tech"><strong>madblocks</strong></a>,
  XPR Network block producer and Metal Blockchain validator.
</p>

---

> **Status: in development.** The core (keys, addresses, the bridge check, building and signing
> transactions) passes BTCVM's own test vectors, the vault works with Windows Hello, and a first version
> of the Windows app runs. Nothing here has been audited, and BTCVM itself is in alpha with a single
> operator. Keep amounts small.

## What it does

[BTCVM](https://metalbtc.com) is a Bitcoin-compatible chain on Metal Blockchain, joined to Bitcoin by a
one-for-one bridge. Payments there are final in about a second and cost a satoshi.

- **Both networks, one key.** Bitcoin and BTCVM share address formats, so your `bc1q…` address is the
  same on both, and the wallet shows both balances.
- **Move BTC to BTCVM.** The wallet pays your personal deposit address from your Bitcoin balance. It
  derives that address itself from the peg signers' keys, which are pinned in the wallet, and refuses one
  from the bridge that doesn't match.
- **Withdraw to Bitcoin.** The wallet pays the bridge's reserve on BTCVM with a tag naming your Bitcoin
  address. The reserve address comes from the same pinned keys.
- **Send** on either network, chosen explicitly each time. **Max** fills in everything a payment can
  move after the fee.
- **Several wallets.** Each is its own key, with its own address, backup and Windows Hello key. Pick one
  from the header; the others appear as destinations, to move funds between them.
- **An address book**, with each address saved for the network you use it on: an exchange that only
  watches Bitcoin never sees what you send it on BTCVM. A destination that looks like a saved address or one
  of your wallets without being it (address poisoning) is flagged in red at review.
- **Choose the Bitcoin fee** for sends and deposits: the bridge's estimate, mempool.space's speeds, or a
  rate of your own, within the wallet's bounds. A low fee never puts funds at risk; it only confirms later,
  and under the network's minimum the payment is refused and nothing leaves. BTCVM's fee is fixed, and the
  bridge sets the fee of a withdrawal's Bitcoin payout.
- **Review, then sign exactly that.** Each payment is shown output by output (which wallet pays, who is
  paid, the change, a withdrawal's Bitcoin destination, the fee and its rate) before anything is signed.
  The signed transaction is read back and must match.

## Security

[SECURITY.md](SECURITY.md) has the full model, including what the wallet does *not* protect against. In
short:

- **The bridge's server isn't trusted with your coins.** It serves balances and broadcasts payments, but
  where deposits and withdrawals go follows from the pinned signer set, each coin's value is read from the
  transaction that created it, and fees are capped. A dishonest server can stall the wallet; it can't
  redirect a payment.
- **The trade-off: if BTCVM's signers change, deposits and withdrawals pause.** A rotation by BTCVM's
  operators looks the same as a hijacked server, so the wallet stops moving coins between the chains until
  it finds the old signers' own move of the funds to the new set and verifies their signatures; then it
  updates itself. Until then it warns you, showing the old and new peg addresses and where to check them (a
  Bitcoin explorer, metalbtc.com, madblocks). Sends keep working. This is deliberate.
- **The key stays on your PC**, encrypted to a Windows Hello key held by the TPM and decrypted only to
  sign. It never reaches the app's web view: a key shown for backup appears in a native dialog, and one
  being imported is read from the clipboard by the app, which then clears it.
- **The core has no `unsafe` code and no networking**, few dependencies, and a hook and a CI job refuse
  any commit containing a private key.

## Architecture

| Layer | Tech | Role | Status |
| --- | --- | --- | --- |
| `core/` | Rust (`btcvm-wallet-core`) | Keys, addresses, deposit addresses, the bridge check, building and signing transactions | Done |
| `vault/` | Rust (`btcvm-wallet-vault`) | The key's vault: AES-256-GCM under a key that only a Windows Hello signature, from a key the TPM holds, produces | Done, checked on real Windows Hello |
| `app/` | Tauri 2 (Rust and WebView2) | The Windows app, in Spanish and English. All networking and signing in Rust; the web view only shows. | First version |

The core must agree byte for byte with BTCVM's web wallet (`chain.js` in
[MetalBlockchain/btc-vm](https://github.com/MetalBlockchain/btc-vm)). Its tests run against the vectors the
web wallet generates and btcd's script engine verifies (`core/tests/vectors/wallet-vectors.json`, from
btc-vm's `cmd/btcvm/testdata`), against BIP 350's address vectors, and against the live bridge's
`/api/info`, saved as a fixture. `scripts/sync-vectors.sh` copies a newer version of the vectors and runs
the tests.

## Building

Requires Rust 1.85 or later. On Windows, the Windows Hello gate needs the MSVC toolchain (Visual Studio
Build Tools with C++). With the GNU toolchain, test the rest with
`cargo test -p btcvm-wallet-core` and `cargo test -p btcvm-wallet-vault --no-default-features`.
`cargo run -p btcvm-wallet-vault --example hello_check` tries Windows Hello on this PC (it asks three times).

```sh
git config core.hooksPath .githooks   # refuse private keys in commits and pushes
cargo test
cargo run -p madblocks-btcvm-wallet   # the app (Windows, with WebView2)
```

## Support madblocks

The wallet is free. If it's useful to you, support the people behind it:

- **Vote for `madblocks`** as a block producer on XPR Network:
  [explorer.xprnetwork.org/vote](https://explorer.xprnetwork.org/vote?producers=madblocks)
- **Delegate to madblocks' validator on Metal Blockchain**, `NodeID-B1hsNPKgi6C89AFybyPFPvDQC2gHxMv7H`
  ([explorer](https://explorer.metalblockchain.org/validators/NodeID-B1hsNPKgi6C89AFybyPFPvDQC2gHxMv7H))
- Follow [@madblocksbp](https://x.com/madblocksbp), and see [madblocks.tech](https://madblocks.tech)

## License

MIT, see [LICENSE](LICENSE). The madblocks name and logo belong to madblocks and aren't covered by it.

This is an independent wallet by madblocks. It is not made or endorsed by Metallicus, and it uses BTCVM's
public bridge API. Its design follows [dogecoin-vm-wallet](https://github.com/paulgnz/dogecoin-vm-wallet)
(MIT), and its transaction code ports BTCVM's web wallet (BSD-3-Clause, Metallicus, Inc.). See
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
