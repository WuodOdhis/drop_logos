# Private Airdrop on Logos LEZ (Prize LP-0003)

A privacy-preserving airdrop / allowlist distributor built on the Logos
Execution Zone (LEZ). The distributor commits a hidden eligibility set (a
Poseidon merkle root, on-chain). Each recipient is minted their allocation into
a private account `D_i` that only they can spend; "claiming" is a native LEZ
privacy transaction from `D_i` into the recipient's own shielded account.
Double-claiming is impossible because spending `D_i` reveals its nullifier on
chain.

## Design in one paragraph

The airdrop program itself is deliberately small — two public instructions,
`initialize_distribution` and `freeze_distribution`. It stores the commitment
(the eligibility root, token definition, totals, distributor, commit time) in a
PDA. It does **not** process claims directly: eligibility is enforced by the
distributor minting only enrolled recipients, and double-spend prevention comes
from LEZ's built-in nullifier set when `D_i` is spent. This keeps the guest
program tiny (well under the sequencer's session limits) and reuses the
protocol's own privacy machinery — shielded receipts, view-key decryption, and
nullifier checks — rather than re-implementing a private transfer inside the
program.

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
├── tests/                  # host integration tests (no zkVM)
├── docs/                   # design + verification notes
├── idl/                    # generated SPEL program IDL
├── scripts/demo.sh         # end-to-end demo
└── logos-airdrop-module/   # Basecamp QML module (Rust provider)
```

## Build

Requires the pinned toolchains (see repo-root `rust-toolchain.toml` and the
risc0 toolchain — `rzup install`, matching `methods/guest`).

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
flow and the on-chain/off-chain data exchanged at each step.

## Tests

```bash
# host integration tests (merkle tree, PDA, serde) — no zkVM
RUSTC_BOOTSTRAP=1 cargo test --manifest-path Cargo.toml --test airdrop_integration

# guest unit tests
cargo test --manifest-path methods/guest/Cargo.toml
```

## IDL

```bash
cargo run --bin generate_idl --manifest-path Cargo.toml > idl/airdrop_program_idl.json
```
