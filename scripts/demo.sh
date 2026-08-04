#!/usr/bin/env bash
# End-to-end private airdrop demo on the LEZ local dev sequencer.
#
# Prerequisites:
#   - The standalone sequencer is running on :3040 (see the repo-root dev.sh).
#   - The airdrop program + host bins are built:
#       cargo risczero build --manifest-path methods/guest/Cargo.toml
#       cargo build --manifest-path Cargo.toml --bins
#
# Flow:
#   1. recipients enroll (each in their own wallet dir),
#   2. the distributor deploys the programs + token + commits the hidden root,
#   3. the distributor funds each hidden allocation account D_i,
#   4. a recipient claims (D_i -> own shielded account) and a double claim fails.
#
# Run every step from the airdrop/ dir (the bins resolve their relative
# data paths from the current working directory):
#   bash scripts/demo.sh [alice,bob,carol]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$SCRIPT_DIR"

RECIPIENTS="${1:-alice,bob,carol}"
DATA_DIR="$SCRIPT_DIR/.logos-airdrop"

export RUST_LOG="${RUST_LOG:-info}"
export RISC0_DEV_MODE="${RISC0_DEV_MODE:-1}"

run() {
  local wallet_dir="$1"; shift
  echo ""
  echo "### [$wallet_dir] $*"
  LEE_WALLET_HOME_DIR="$wallet_dir" RUSTC_BOOTSTRAP=1 cargo run --quiet --bin "$@" 2>&1
}

wallet() {
  echo "$DATA_DIR/wallets/$1"
}

echo "== Private Airdrop Demo =="
echo "  recipients: $RECIPIENTS"
echo "  data dir:   $DATA_DIR"
echo ""

# --- 0. Fresh state -----------------------------------------------------------
# (comment out to keep previous runs' accounts/enrollments)
rm -rf "$DATA_DIR"

# --- 1. Recipients enroll -----------------------------------------------------
for name in ${RECIPIENTS//,/ }; do
  run "$(wallet "$name")" airdrop_enroll -- "$name" 1000000
done

# --- 2. Distributor deploys + commits the hidden root -------------------------
run "$(wallet distributor)" airdrop_deploy -- 1

# --- 3. Distributor funds each allocation into D_i ----------------------------
run "$(wallet distributor)" airdrop_fund

# --- 4. Recipient claims ------------------------------------------------------
for name in ${RECIPIENTS//,/ }; do
  run "$(wallet "$name")" airdrop_claim -- --name "$name"
done

# --- 5. Verify ----------------------------------------------------------------
run "$(wallet distributor)" airdrop_status

echo ""
echo "== Demo complete =="

