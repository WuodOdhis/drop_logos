#!/usr/bin/env bash
# Sequencer-backed integration tests.
#
# Builds everything needed (guest ELF via the local non-docker risc0 path, the
# host bins), provisions the pinned LEZ standalone sequencer, and runs
# `tests/sequencer_integration.rs` which spawns the sequencer itself on an
# ephemeral port with a fresh temp data dir.
#
# Usage (from the airdrop/ dir):
#   bash scripts/run_integration_tests.sh
#
# Env overrides:
#   LEZ_SEQUENCER_BIN      path to a prebuilt sequencer_service (defaults to the
#                          cache path below, built if missing)
#   LEZ_SEQUENCER_CONFIG   path to the standalone sequencer config
#   LEZ_SKIP_SEQUENCER_BUILD=1  don't rebuild the sequencer if present
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$SCRIPT_DIR"

export RUSTC_BOOTSTRAP=1
export RISC0_DEV_MODE="${RISC0_DEV_MODE:-1}"

LEZ_SEQUENCER_BIN="${LEZ_SEQUENCER_BIN:-/home/badman/.cache/logos-lez-rln/sequencer-src/target/release/sequencer_service}"

echo "== Building the guest ELF (local risc0 build, no Docker) =="
cargo build --manifest-path methods/Cargo.toml

echo ""
echo "== Building host bins =="
cargo build --bins

if [[ ! -x "$LEZ_SEQUENCER_BIN" && "${LEZ_SKIP_SEQUENCER_BUILD:-0}" != "1" ]]; then
  echo ""
  echo "== Sequencer binary missing; building it (this takes a while) =="
  seq_src="$(dirname "$(dirname "$LEZ_SEQUENCER_BIN")")"
  if [[ -d "$seq_src" ]]; then
    CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" cargo build --release \
      --manifest-path "$seq_src/sequencer/service/Cargo.toml"
  else
    echo "Sequencer source not found at $seq_src; set LEZ_SEQUENCER_BIN." >&2
    exit 1
  fi
fi

echo ""
echo "== Running the sequencer integration test =="
RISC0_DEV_MODE=1 cargo test --test sequencer_integration -- --ignored

echo ""
echo "== Integration tests complete =="
