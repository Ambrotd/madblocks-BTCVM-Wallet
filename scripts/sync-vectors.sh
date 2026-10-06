#!/usr/bin/env bash
# Copies the wallet test vectors from a btc-vm checkout (default ../btc-vm)
# and a dogecoin-vm one (default ../dogecoin-vm), where BTCVM's and
# DogecoinVM's web wallets generate them and btcd's script engine checks
# them. The core's tests must then pass unchanged.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BTC="${1:-$ROOT/../btc-vm}/cmd/btcvm/testdata/wallet-vectors.json"
DOGE="${2:-$ROOT/../dogecoin-vm}/cmd/dogevm/testdata/wallet-vectors.json"
cp "$BTC" "$ROOT/core/tests/vectors/wallet-vectors.json"
cp "$DOGE" "$ROOT/core/tests/vectors/doge-wallet-vectors.json"
(cd "$ROOT" && cargo test --quiet -p btcvm-wallet-core)
