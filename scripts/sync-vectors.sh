#!/usr/bin/env bash
# Copies the wallet test vectors from a btc-vm checkout (default ../btc-vm),
# where BTCVM's web wallet generates them and btcd's script engine checks
# them. The core's tests must then pass unchanged.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="${1:-$ROOT/../btc-vm}/cmd/btcvm/testdata/wallet-vectors.json"
cp "$SRC" "$ROOT/core/tests/vectors/wallet-vectors.json"
(cd "$ROOT" && cargo test --quiet -p btcvm-wallet-core)
