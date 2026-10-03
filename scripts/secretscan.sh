#!/usr/bin/env bash
# Refuses private keys in what is committed or pushed: WIFs, extended private
# keys (xprv…) and PEM private keys.
#
#   scripts/secretscan.sh staged    what is about to be committed (pre-commit)
#   scripts/secretscan.sh history   every line ever added, in every commit
#                                   (pre-push and CI)
#
# The test vectors' keys are public, SHA-256 of labels like "btcvm vector
# key 1", and are allowed by path below.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

ALLOW='^core/tests/vectors/wallet-vectors\.json$'
B58='1-9A-HJ-NP-Za-km-z'
# A WIF: 5… (51 characters) or K…/L… (52) on mainnet, 9…/c… on testnet.
WIF="(^|[^$B58])[5KLc9][$B58]{50,51}([^$B58]|\$)"
XPRV="[tuvxyz]prv[$B58]{100,}"
PEM='-----BEGIN ([A-Z0-9]+ )*PRIVATE KEY-----'

# Turns a unified diff into "path<TAB>added line" pairs.
added_lines() {
  awk '
    /^diff --git / { header = 1; next }
    /^@@/ { header = 0; next }
    header && /^\+\+\+ / { path = substr($0, 7); next }
    !header && /^\+/ { print path "\t" substr($0, 2) }
  '
}

# Reads "path<TAB>line" pairs and reports those holding a key. A WIF-shaped
# run made only of hex digits is part of a hash, not a key.
report() {
  local found=0 path line m
  while IFS=$'\t' read -r path line; do
    [[ $path =~ $ALLOW ]] && continue
    if [[ $line =~ $XPRV || $line =~ $PEM ]]; then
      echo "secretscan: a private key in $path" >&2
      found=1
      continue
    fi
    while read -r m; do
      m=$(printf '%s' "$m" | tr -cd "$B58")
      [[ -z $m || $m =~ ^[0-9a-fA-F]+$ ]] && continue
      echo "secretscan: what looks like a WIF private key (${m:0:4}…) in $path" >&2
      found=1
    done < <(printf '%s\n' "$line" | grep -oE "$WIF" || true)
  done
  return $found
}

case "${1:-}" in
  staged) diff=(git diff --cached --no-color --no-ext-diff --unified=0 --diff-filter=ACMR) ;;
  history) diff=(git log --all -p --no-color --no-ext-diff --unified=0 --format=) ;;
  *) echo "usage: $0 staged|history" >&2; exit 2 ;;
esac

if ! "${diff[@]}" | added_lines | { grep -E -e "$WIF" -e "$XPRV" -e "$PEM" || true; } | report; then
  echo "secretscan: refusing. Remove the key; if it was ever real, treat it as leaked and move its funds." >&2
  exit 1
fi
