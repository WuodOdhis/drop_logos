# Integration Guide: using the Private Airdrop program from another LEZ module

This guide is for developers who want to integrate the airdrop program into
their own Logos Execution Zone (LEZ) module, wallet, or service. It covers the
program's public surface (`airdrop-core`), the host-side helpers
(`src/airdrop/client.rs`), a minimal code example, and the exact CLI sequences
for every step. Everything here is real API surface that the repo's bins and
tests already use; there are no hypothetical calls.

## 1. What a third-party module gets

The airdrop program exposes two public instructions and a single on-chain
account:

- `Instruction::InitializeDistribution { distribution_id, root,
  token_definition, total_allocation, num_eligible }` (distributor-signed).
- `Instruction::FreezeDistribution { distribution_id }` (distributor-signed).
- `DistributionState`, a 216-byte Borsh record at the distribution PDA.

An integrating module most commonly wants to: (a) compute the distribution PDA
for a known `distribution_id`, (b) read and decode `DistributionState`, and
(c) verify the committed root against its own copy of the enrollment leaves.
All three are pure, `no_std`-friendly operations available from `airdrop-core`.

## 2. The `airdrop-core` SDK surface

`airdrop-core` is a `no_std`-capable crate shared by the guest and the host.
Integrate it from a `std` host as:

```toml
airdrop-core = { path = "../airdrop-core" }
```

or from a `no_std` guest as:

```toml
airdrop-core = { path = "../airdrop-core", default-features = false }
```

Public items (re-exports in `airdrop-core/src/lib.rs`):

| Item | Purpose |
|---|---|
| `DistributionState` | The on-chain distribution config (root, token definition, totals, distributor, commit time, active flag). Decode with `borsh::from_slice` on the PDA account data. |
| `Instruction` | The serde `Instruction` enum passed between host and guest. |
| `distribution_pda_seed(id)` / `distribution_seed(id)` / `label_seed` / `u64_seed` / `combine_seeds` | PDA seed derivation. `distribution_seed(id) = SHA-256("distribution" || u64_le(id))`, mirroring the guest exactly (see `docs/design.md` §4). |
| `SparseMerkleTree` (in `merkle`) | Poseidon-BN254 sparse merkle tree over `H(D_i)` leaves. `with_default_depth()` (depth 32), `insert`, `root`, `path`, `verify_inclusion`, `hash_single`. |
| `constants` | `CLOCK_50_ACCOUNT_ID_BYTES`, `DISTRIBUTION_LABEL`, `DEFAULT_TREE_DEPTH`, `ZERO_COMMITMENT`. |

The tree hashing crate is pinned to `rust-poseidon-bn254-pure` at the same
revision the LEZ guest uses (`rev 49e1042`), so any host computing roots or
leaves is bit-identical to the guest. Do not swap that hash crate.

## 3. Host-side helpers (`src/airdrop/client.rs`)

The host crate (`airdrop`, `src/lib.rs` exposes `airdrop::airdrop::*`) wraps
the wallet and sequencer RPC. Reusable helpers:

| Helper | What it does |
|---|---|
| `init_wallet()` | Loads or creates the `WalletCore` from the wallet config + storage paths (`LEE_WALLET_HOME_DIR`). |
| `load_program()` | Reads the deploy ELF from `methods/target/.../airdrop.bin` and returns the `Program`. |
| `distribution_account(program, id)` | `AccountId` of the distribution PDA. |
| `read_distribution(wallet, program, id)` | Fetches + Borsh-decodes `DistributionState`. |
| `initialize_distribution(wallet, program, id, distributor, root, token_definition, total, num)` | Sends the commit tx (distributor signs). |
| `freeze_distribution(wallet, program, id, distributor)` | Sends the freeze tx. |
| `ensure_program_deployed` / `deploy_builtin_program` | Idempotent program deploys (airdrop ELF / builtin token + authenticated-transfer). |
| `eligibility_leaf(account_id)` / `eligibility_root(leaves)` | `H(D_i)` and the SMT root. |
| `hex_account` / `hex32` / `parse_hex32` / `hex_program_id` | Hex helpers for manifests and CLI output. |
| `wait_for_block_seal()` / `wait_for_account_data()` | Polling helpers; both are env-tunable for CI (`LEZ_AIRDROP_BLOCK_SEAL_SECS`, `LEZ_AIRDROP_ACCOUNT_WAIT_ATTEMPTS`). |

The privacy transactions (mint into `D_i`, transfer `D_i -> main`) are native
LEZ token-program calls, not airdrop-guest instructions. They go through the
`wallet` crate's `program_facades::token::Token`:

- `Token::send_mint_transaction_private_foreign_account(def, npk, vpk,
  identifier, amount)` mints into `D_i` without needing `D_i`'s secret key.
- `Token::send_transfer_transaction_private_owned_account(from, to, amount)`
  is the claim.

## 4. Minimal code example

Read the committed root for a distribution and verify an enrollment's leaf
against it (no wallet, no sequencer needed for verification):

```rust
use airdrop::airdrop::client::{parse_hex32, hex32};
use airdrop::airdrop::types::Enrollment;
use airdrop_core::DistributionState;
use airdrop_core::merkle::SparseMerkleTree;

/// Distribution PDA seed, guest-identical.
fn pda_seed(distribution_id: u64) -> [u8; 32] {
    airdrop_core::distribution_pda_seed(distribution_id)
}

/// Decode a distribution account's data into `DistributionState`.
fn decode_state(data: &[u8]) -> DistributionState {
    borsh::from_slice(data).expect("decode DistributionState")
}

/// Verify one enrollment leaf against the committed root.
fn verify_leaf(root: &[u8; 32], enrollment: &Enrollment, index: u64) -> bool {
    let leaf = parse_hex32(&enrollment.leaf, "leaf");
    let mut tree = SparseMerkleTree::with_default_depth();
    tree.insert(index, leaf);
    // A real caller re-inserts the full set; the tree recomputes the root from
    // the same leaves, and `verify_inclusion` checks the leaf at `index`.
    let path = tree.path(index);
    SparseMerkleTree::verify_inclusion(root, &leaf, &path, index)
        && tree.root() == *root
}
```

For a full on-chain read + send flow, mirror what the bins do: `init_wallet()`,
`load_program()`, then `read_distribution` (read) or `initialize_distribution`
(send), using `Message::try_new` + `WitnessSet::for_message` +
`PublicTransaction::new` as in `client.rs`. See `docs/verification.md` for the
host transaction plumbing notes.

## 5. Exact CLI sequences

All bins run from the `airdrop/` directory (they resolve `.logos-airdrop` and
`methods/target` relative to the CWD). The wallet for each role is selected via
`LEE_WALLET_HOME_DIR`. `RISC0_DEV_MODE` selects dev-mode (fast) vs real
proofs (slow).

### Recipient: enroll

```bash
# creates D_i + the shielded claim account, writes .logos-airdrop/enrollments/<name>.json
LEE_WALLET_HOME_DIR=wallets/alice RISC0_DEV_MODE=1 \
  RUSTC_BOOTSTRAP=1 cargo run --bin airdrop_enroll -- alice 1000000
```

### Distributor: deploy + commit root

```bash
# reads all enrollments, deploys programs, creates the token, commits the root
LEE_WALLET_HOME_DIR=wallets/distributor RISC0_DEV_MODE=1 \
  RUSTC_BOOTSTRAP=1 cargo run --bin airdrop_deploy -- 1
```

### Distributor: fund

```bash
# mints each allocation into its D_i
LEE_WALLET_HOME_DIR=wallets/distributor RISC0_DEV_MODE=1 \
  RUSTC_BOOTSTRAP=1 cargo run --bin airdrop_fund
```

### Recipient: claim (then double claim is rejected)

```bash
LEE_WALLET_HOME_DIR=wallets/alice RISC0_DEV_MODE=1 \
  RUSTC_BOOTSTRAP=1 cargo run --bin airdrop_claim -- --name alice
```

### Anyone: verify

```bash
# recomputes the root from enrollments and checks it against the on-chain state
LEE_WALLET_HOME_DIR=wallets/distributor RISC0_DEV_MODE=1 \
  RUSTC_BOOTSTRAP=1 cargo run --bin airdrop_status
```

The scripted equivalent, `scripts/demo.sh alice,bob,carol`, runs all of the
above with fresh state and asserts the double-claim failure.

## 6. Error handling

The airdrop guest returns deterministic codes for invalid-input and
authorization failures. See `docs/errors.md` for the full table (1..8) and the
note that claim/double-claim failures come from the LEZ token runtime (spent
nullifier), not from the airdrop guest.

## 7. Reproducibility notes for integrators

- Pins: host toolchain `1.94.0` (`rust-toolchain.toml`), risc0 `3.0.5` guest
  toolchain `r0.1.88.0`, LEZ `v0.2.0-rc6` crates, SPEL at the pinned revision,
  `rust-poseidon-bn254-pure` at `rev 49e1042`.
- `RUSTC_BOOTSTRAP=1` is required for all builds/tests because of the pinned
  hash crate's `#![feature(generic_const_exprs)]`.
- The deploy ELF is produced by `cargo build --manifest-path methods/Cargo.toml`
  (local risc0 build; no Docker needed) and stripped by `methods/build.rs`.
- CI runs all host tests, clippy (`-D warnings`), the guest host-side tests,
  and the sequencer-backed integration test. See `.github/workflows/ci.yml`.
