# Private Airdrop on Logos LEZ (Prize LP-0003)

A privacy-preserving airdrop / allowlist distributor built on the Logos
Execution Zone (LEZ). The distributor commits a hidden eligibility set (a
Poseidon merkle root, on-chain). Each recipient is minted their allocation into
a private account `D_i` that only they can spend; "claiming" is a native LEZ
privacy transaction from `D_i` into the recipient's own shielded account.
Double-claiming is impossible because spending `D_i` reveals its nullifier on
chain.

## Design in one paragraph

The airdrop program itself is deliberately small: two public instructions,
`initialize_distribution` and `freeze_distribution`. It stores the commitment
(the eligibility root, token definition, totals, distributor, commit time) in a
PDA. It does **not** process claims directly: eligibility is enforced by the
distributor minting only enrolled recipients, and double-spend prevention comes
from LEZ's built-in nullifier set when `D_i` is spent. This keeps the guest
program tiny (well under the sequencer's session limits) and reuses the
protocol's own privacy machinery: shielded receipts, view-key decryption, and
nullifier checks, rather than re-implementing a private transfer inside the
program.

For the full mechanism, on-chain/off-chain data flows, and assumptions, see
`docs/design.md`. For the formal threat model (who learns what, distributor
trust, "unlinkable" defined relative to a stated adversary), see
`docs/privacy-model.md`. For deterministic program error codes, see
`docs/errors.md`.

## Roles

- **Distributor** runs `airdrop_deploy` (commits the root, deploys the token +
  airdrop programs) and `airdrop_fund` (mints each allocation into the
  recipients' hidden accounts).
- **Recipient** runs `airdrop_enroll` (creates `D_i`, publishes an enrollment
  file containing only public keys + identifier) and `airdrop_claim` (spends
  `D_i` into their own shielded account).

## Repo layout

```
airdrop/
├── Cargo.toml              # host workspace: lib + 6 bins
├── airdrop-core/           # shared no_std crate (guest + host)
│   └── src/{state,instruction,pda,merkle,constants}.rs
├── methods/
│   ├── Cargo.toml          # risc0_build embedding
│   └── guest/              # SPEL program + handlers (the zkVM guest)
├── src/
│   ├── bin/                # enroll, deploy, fund, claim, status, generate_idl
│   └── airdrop/            # client/pda/types host helpers
├── tests/                  # host integration tests + sequencer-backed E2E
├── docs/                   # design, privacy model, verification, errors
├── idl/                    # generated SPEL program IDL
└── scripts/                # demo.sh + run_integration_tests.sh
```

## Build

Requires the pinned toolchains (see repo-root `rust-toolchain.toml` and the
risc0 toolchain: `rzup install`, matching `methods/guest`).

```bash
# reproducible deploy guest binary (the deploy artifact)
cargo risczero build --manifest-path methods/guest/Cargo.toml

# host bins + strips the deploy binary under the per-tx size cap
RUSTC_BOOTSTRAP=1 cargo build --manifest-path Cargo.toml --bins
```

`RUSTC_BOOTSTRAP=1` is required because the pinned `rust-poseidon-bn254-pure`
(hash crate shared with the LEZ guest) still uses `#![feature(generic_const_exprs)]`.

## Run the demo

Start the local dev sequencer first (repo-root `dev.sh`, port 3040), then:

```bash
bash scripts/demo.sh alice,bob,carol
```

Each recipient enrolls in their own wallet dir; the distributor deploys, funds,
and the demo asserts a double claim fails. See `docs/design.md` for the full
flow.

To run the full flow against a **standalone sequencer spawned by the test
itself** (the prize-required integration test):

```bash
bash scripts/run_integration_tests.sh
```

This builds the guest ELF + bins, provisions the pinned standalone sequencer,
starts it on an ephemeral port with a fresh temp data dir, and asserts
enroll, deploy, fund, claim, double-claim rejection, and root verification.
It is the same job CI runs.

## Deployment and program addresses

The airdrop and token programs are deployed by the distributor's own wallet at
run time (see `airdrop_deploy`), so program IDs are not hardcoded anywhere in
this repository; they are deterministic per deployed bytecode and are recorded
in the run manifest written to `.logos-airdrop/run.json` and echoed by
`airdrop_status`.

Live testnet deployments, their verified program IDs, and the deployment tx
hashes are recorded in `docs/testnet.md` together with the reproducibility
script (`scripts/testnet_run.sh`). See that file for the current, real
addresses. Until a testnet deployment exists, this README deliberately does not
claim any program address.

## Tests

```bash
# host integration tests (merkle tree, PDA, serde): no zkVM
RUSTC_BOOTSTRAP=1 cargo test --manifest-path Cargo.toml --test airdrop_integration

# guest host-side unit tests (also need RUSTC_BOOTSTRAP=1 for the pinned
# rust-poseidon-bn254-pure hash crate)
RUSTC_BOOTSTRAP=1 cargo test --manifest-path methods/guest/Cargo.toml

# sequencer-backed end-to-end test (needs scripts/run_integration_tests.sh)
RISC0_DEV_MODE=1 cargo test --test sequencer_integration -- --ignored
```

## IDL

```bash
cargo run --bin generate_idl --manifest-path Cargo.toml > idl/airdrop_program_idl.json
```

## Integration guide

Third-party LEZ modules that want to use the airdrop program (read the
distribution state, compute PDA addresses, build enrollments) should read
`docs/integration-guide.md`, which covers the `airdrop-core` SDK surface, the
host `client.rs` helpers, a minimal code example, and the exact CLI sequences.

## License

Licensed under either of:

- Apache License, Version 2.0 (`LICENSE-APACHE-v2`)
- MIT license (`LICENSE-MIT`)

at your option.
