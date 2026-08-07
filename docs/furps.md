# FURPS Self-Assessment: Private Airdrop / Allowlist Distributor

This is the required FURPS self-assessment. It follows the standard
Functionality, Usability, Reliability, Performance, Supportability structure.
Every score is evidence-backed: it cites this repo's own docs, tests, and CI.
Where an item is not yet measured, that is stated explicitly rather than
guessed. Performance numbers are marked provisional until Item 9 (benchmarks)
lands real CU and proof-generation figures.

Scores use: **Solid** (done, tested), **Provisional** (design/implementation
exists, measurement or deployment pending), **Gap** (acknowledged deficiency).

---

## Functionality

| Aspect | Score | Evidence |
|---|---|---|
| Two-instruction airdrop program (`initialize_distribution`, `freeze_distribution`) | Solid | Guest `methods/guest/src/lib.rs`; design `docs/design.md` §4. |
| Eligibility enforced by distributor minting only enrolled recipients | Solid | `airdrop_fund` mints per-enrollment `D_i`; verified in integration test. |
| Double-claim rejection via LEZ nullifier set | Solid | `tests/sequencer_integration.rs` asserts second claim fails; `scripts/demo.sh` same. |
| Hidden eligibility set (private token definition + supply, `D_i` shielded accounts) | Solid | `docs/design.md` §5; `airdrop_deploy` creates private accounts; `docs/privacy-model.md` A4. |
| Root commitment and inclusion verification | Solid | Guest verifies `verify_inclusion` before claim; `airdrop_status` recomputes root; `docs/design.md` §4. |
| Deterministic error codes | Solid | Guest returns `Err(AirdropError::...)` codes 1-8; `docs/errors.md`. |
| PDA derivation guest/host agreement | Solid | Shared `airdrop-core::pda`; `docs/design.md` §4. |
| Idempotent program deployment | Solid | `ensure_program_deployed` / `deploy_builtin_program`; `tests/sequencer_integration.rs` tolerates `ProgramAlreadyExists`. |

## Usability

| Aspect | Score | Evidence |
|---|---|---|
| Five purpose-built CLI binaries (enroll, deploy, fund, claim, status) | Solid | `src/bin/airdrop_{enroll,deploy,fund,claim,status}.rs`; README CLI sequences. |
| Wallet isolation via `LEE_WALLET_HOME_DIR` | Solid | Each role runs in its own wallet dir; `scripts/demo.sh`. |
| One-command demo | Solid | `bash scripts/demo.sh alice,bob,carol` asserts the full flow. |
| Third-party integration guide | Solid | `docs/integration-guide.md` (SDK surface, host helpers, code example, CLI sequences). |
| Seamless dev-mode vs real-proof mode | Solid | `RISC0_DEV_MODE` toggles; CI + scripts document both. |
| GUI (Basecamp QML module) | Gap | The former `logos-airdrop-module/` was removed from this repo (out of scope for the core solution); not claimed. |

## Reliability

| Aspect | Score | Evidence |
|---|---|---|
| End-to-end test against a real standalone sequencer | Solid | `tests/sequencer_integration.rs`, green in CI (run `31137520872`). |
| Host unit tests (merkle, PDA, serde) | Solid | `tests/airdrop_integration.rs`; CI `test` job. |
| Guest host-side unit tests | Solid | `methods/guest` tests; CI `test` job. |
| Idempotency of deploy retries | Solid | Integration test tolerates `ProgramAlreadyExists`; `wait_for_block_seal`. |
| Failure modes surface as deterministic errors | Solid | `docs/errors.md`; guest never panics on invalid input. |
| Long-running durability (sequencer crash recovery) | Provisional | Relies on sequencer's own persistence; not fault-injected in this repo's tests. |
| Concurrent claimants correctness | Provisional | Locked by sequencer processing; no load test yet (Item 12 backlog work). |

## Performance

| Aspect | Score | Evidence |
|---|---|---|
| Guest fits sequencer session limits (dev mode) | Solid | `RISC0_DEV_MODE=1` full flow passes; guest `r0vm`/stripped ELF ~277 KB. |
| Real succinct proof generation | Provisional | `RISC0_DEV_MODE=0` not yet run (7.2 GB RAM OOM risk); Item 10. |
| CU cost per instruction | Provisional | Not measured; Item 9. |
| Proof generation wall time | Provisional | Not measured; Item 9. |
| Claim latency on testnet | Provisional | No testnet deployment yet; Item 11. |

## Supportability

| Aspect | Score | Evidence |
|---|---|---|
| Pinned host + guest toolchains | Solid | `rust-toolchain.toml` (host 1.94.0), risc0 3.0.5 guest toolchain; `docs/verification.md`. |
| CI: tests, clippy -D warnings, build, integration job | Solid | `.github/workflows/ci.yml`, all four jobs green. |
| Error-code reference | Solid | `docs/errors.md`. |
| Design + privacy model + verification docs | Solid | `docs/design.md`, `docs/privacy-model.md`, `docs/verification.md`. |
| Reproducible builds / artifact scripts | Solid | `scripts/run_integration_tests.sh`; `methods/build.rs` strips ELF. |
| License metadata | Solid | `LICENSE-APACHE-v2`, `LICENSE-MIT`, root `LICENSE`. |
| Live testnet deployment + verified program IDs | Provisional | `docs/testnet.md` + `scripts/testnet_run.sh` pending (Item 11). |
| Formal verification (proof of correctness of the guest) | Provisional | Not started; Item 15. |

---

## Overall

Strengths: the core privacy mechanism is small, tested end-to-end against the
real sequencer, deterministic, and documented with a formal threat model.

Gaps (each tracked as a remediation item): real succinct proofs (10), testnet
deployment (11), CU/benchmark numbers (9), concurrency/backlog (12), formal
verification (15). The Performance and Supportability tables will be updated
with real numbers once Items 9, 10, and 11 land; this document is the place
those numbers will be recorded.
