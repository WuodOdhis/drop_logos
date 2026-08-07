# Remediation Plan: LP-0003 (Private Allowlist / Airdrop Distributor)

Status: **PLAN**: written after `ASSESSMENT_ACKNOWLEDGEMENT.md`, before any
fix is implemented.

Goal: turn the current "works locally, fails the rubric" state into a
submission that satisfies the LP-0003 success criteria and is honestly
verifiable: not a facade. Each item below maps to a specific prize
requirement and a specific finding from the assessment, is ordered by
dependency (not by ease), and ends with a definition of done.

The two anchor facts that shape the whole plan:

1. **The prize gates on testnet + real proofs.** `RISC0_DEV_MODE=1` (the
   current demo default, `scripts/demo.sh:28`) is only acceptable for
   development. The prize requires a demo that runs with `RISC0_DEV_MODE=0`,
   ≥2 testnet distributions, ≥20 unique claims, and a narrated video showing
   real proof generation. Everything else is subordinate to making the real
   proving path work.
2. **The privacy model is fixable by documentation, not redesign.** As
   corrected in the acknowledgment, LP-0003 scopes claim privacy to on-chain
   observers and explicitly allows distributor knowledge as a trade-off axis.
   The distributor-trust property is a *documentation* obligation, not a
   defect to engineer away.

---

## Ground rules

- **No fabricated evidence.** Every claim (program IDs, tx hashes, claim
  counts, CU costs, proof times) must come from a real run whose artifacts are
  committed to the repo (logs, JSON manifests, receipts). If a number cannot be
  produced, the plan item is NOT done.
- **Do the hard thing first.** Testnet + real proofs are the risky,
  irreversible path; they are scheduled early so a failure surfaces before
  weeks of polish work.
- **CI must be green before new evidence is generated**, so evidence is
  reproducible by evaluators, not just by us.
- **Each item is independently shippable.** If we run out of time, the items
  are ordered so the cheapest P0s land first.

---

## Item 1: LICENSE files + root license metadata

**Prize req:** "Public repository... under MIT or Apache-2.0."
**Assessment finding:** no `LICENSE*`; root `Cargo.toml` has no `license` field.

**Work**
1. Add `LICENSE-MIT` and `LICENSE-APACHE-v2` (mirroring the parent
   `logos-lez-rln` repo's texts, which the workspace is derived from).
2. Add `license = "MIT OR Apache-2.0"` to the root `Cargo.toml`, and to any
   crate missing it (`methods/guest/Cargo.toml` has none).
3. Add a `README.md` "License" section pointing at both files.

**DoD:** `git grep -l "license" Cargo.toml airdrop-core/Cargo.toml methods/guest/Cargo.toml` all set; LICENSE files tracked and pushed.

**Effort:** ~15 min.

---

## Item 2: `rust-toolchain.toml` + risc0 toolchain pin

**Prize req:** reproducible build (implied by "evaluators clone and run");
`verification.md` already *references* a `rust-toolchain.toml` that isn't in the repo.
**Assessment finding:** toolchain file missing.

**Work**
1. Commit `rust-toolchain.toml`:
   ```toml
   [toolchain]
   channel = "1.94.0"
   profile = "default"
   components = ["rustfmt", "clippy", "rust-src"]
   ```
2. Document the risc0 guest toolchain (`r0.1.88.0` per `verification.md`) in
   `methods/guest/README.md` or the root README, including `rzup install`.
3. Note: the guest has no custom release profile (see
   `methods/guest/Cargo.toml:5-8`): the doc must state that this is load-bearing
   for the cycle budget.

**DoD:** fresh clone on a clean machine selects 1.94.0 automatically; guest toolchain documented.

**Effort:** ~30 min.

---

## Item 3: CI workflow (tests + clippy + build)

**Prize req:** "CI must be green on the default branch"; "end-to-end
integration tests run against a LEZ sequencer (standalone mode) and are
included in CI."
**Assessment finding:** no `.github/` at all.

**Work**
1. Add `.github/workflows/ci.yml` with three jobs:
   - **host:** `cargo test --manifest-path Cargo.toml` + `cargo clippy`
     (with `RUSTC_BOOTSTRAP=1` as documented).
   - **guest:** `cargo test --manifest-path methods/guest/Cargo.toml`
     (host-side unit tests only: no risc0 toolchain needed to *compile* tests
     on the host is NOT assumed; verify whether guest tests need the target).
   - **check:** `cargo fmt --check` if formatting is currently clean, else add
     a `rustfmt.toml` normalization step first.
2. Cache `~/.cargo` and `target/` between runs; cap `CARGO_BUILD_JOBS=2` to
   avoid OOM on the 7.2 GB CI runner (same reason as locally).
3. Keep the risc0 guest *build* (`risczero build`) out of default CI until the
   toolchain is reliably installable in CI; gate it behind a `paths:` trigger
   on `methods/**` or a manual workflow. Document this decision.

**DoD:** workflow file exists; all three jobs pass on a fresh push; badge in README.

**Effort:** ~2-4 h.

---

## Item 4: Sequencer-backed integration tests

**Prize req:** "End-to-end integration tests run against a LEZ sequencer
(standalone mode) and are included in CI."
**Assessment finding:** `tests/airdrop_integration.rs` explicitly runs "without
the zkVM or a sequencer."

**Work**
1. Add a script `scripts/run_integration_tests.sh` that: builds the standalone
   sequencer (or pulls the pinned `sequencer_service` binary), starts it with a
   temp data dir on a non-default port, runs a full enroll→deploy→fund→claim→
   double-claim flow against it, and tears it down.
2. Encode the flow as `tests/sequencer_integration.rs` asserting the same
   invariants the demo asserts (D_i funded, claim balance moves, double claim
   rejected), reading a committed manifest rather than hardcoded hashes.
3. Wire it into CI as a separate job (depends on Item 3).
4. Keep it `RISC0_DEV_MODE=1` for CI speed; the real-proof job is Item 10.

**DoD:** test passes against a freshly started standalone sequencer; CI job green.

**Effort:** ~4-8 h. (Risk: sequencer start-time and port conflicts.)

---

## Item 5: Deterministic error codes (guest)

**Prize req:** "The verifier program returns deterministic, documented error
codes for all invalid-proof and double-claim scenarios."
**Assessment finding:** `methods/guest/src/handlers.rs` uses `assert!` strings;
no error enum.

**Work**
1. Introduce an `AirdropError` enum in `airdrop-core/src/error.rs` (or in the
   guest) with variants for every current `assert!`/`expect` in `handlers.rs`
   (`:34, :52, :53, :57, :58, :92, :100`) plus the double-claim case.
2. Add `Display` + serde + a stable numeric/string code per variant, and a
   `docs/errors.md` table mapping code → condition → recovery action.
3. Replace `assert!` panics with `Err(AirdropError::...)` in the guest
   handlers, since SPEL/LEZ surfaces guest panics as
   `TransactionBuildError(ProgramProveFailed(...))` (seen in
   `airdrop_claim.rs` double-claim rejection). Confirm the error path actually
   propagates the code on-chain (depends on SPEL macros: verify first).
4. Update the CLI bins to print the code, not just the panic string.

**DoD:** no bare `assert!` remains in handlers.rs; every failure mode has a
documented code; `docs/errors.md` exists.

**Effort:** ~2-4 h. (Risk: SPEL may force panics; if so, document the mapping
of panic strings to codes in the host layer instead: but verify first.)

---

## Item 6: Formal privacy model / threat model write-up

**Prize req:** "documents its full privacy model: what on-chain observers
learn, what the distributor learns, at which points... identity information is
revealed or withheld, and where trade-offs or residual leakage remain. Claims
of privacy must be precise: 'unlinkable' must be defined relative to a stated
threat model."
**Assessment finding:** §7 is a short bullet list; no adversary definitions;
"unlinkable" never defined; distributor view unstated; no trusted-setup
statement.

**Work**: this is the documentation item that most needs care, so it gets its
own phase rather than being bundled:
1. Write `docs/privacy-model.md` with, at minimum:
   - **Adversary definitions:** observer (reads chain), distributor (holds
     enrollment files + mint records + nullifier set), outside attacker
     (targeting/excluding/front-running), with explicit capabilities per
     adversary.
   - **Stage-by-stage "who learns what" table:** enroll, deploy/commit, fund,
     claim, post-claim: for each of observer / distributor / other recipients.
   - **Explicit distributor trust statement:** the distributor can link every
     `D_i` to a recipient and observe each claim; this is a stated trust
     assumption and a valid point on the trade-off axis the prize names.
   - **Formal definition of "unlinkable"** relative to the *observer*
     adversary: what distribution of transcripts is required, and the honest
     caveat that claim-time amounts and timings are visible.
   - **Trusted setup:** state plainly that Poseidon + risc0 have no trusted
     setup, or if any auxiliary setup exists, name it.
   - **Residual leakage:** timing correlation, amount leakage, set-size
     leakage (count of mints reveals eligibility-set size), the
     observer-can't-but-distributor-can asymmetry.
2. Cross-reference `design.md` §7/§8 and fix any contradiction.

**DoD:** an evaluator can read `docs/privacy-model.md` alone and answer every
"who learns what" question the prize lists. The word "unlinkable" appears
*with* a definition.

**Effort:** ~3-5 h.

---

## Item 7: README + integration guide

**Prize req:** "README documents end-to-end usage: deployment steps, program
addresses, and step-by-step instructions for interacting with the program via
CLI and Basecamp app." + "Write-up covering... integration instructions."
**Assessment finding:** README has no program addresses, no Basecamp usage, no
integration guide.

**Work**
1. Rewrite README "Deployment" section to show the real program ID(s) once
   testnet deployments exist (Item 11): do NOT hardcode placeholder IDs.
2. Add `docs/integration-guide.md`: how a third-party LEZ module uses the
   airdrop program (SDK surface of `airdrop-core` + the host `client.rs`
   helpers), a minimal code example, and the exact CLI sequences.
3. Fix the phantom `logos-airdrop-module/` reference in the README layout
   block (`README.md:50`): either implement it (Item 14) or remove the line.
   A stub in the layout with no content is a reproducibility defect.

**DoD:** README + integration guide are internally consistent and match the
actual repo tree.

**Effort:** ~2-3 h (excluding the module decision).

---

## Item 8: FURPS self-assessment

**Prize req:** "FURPS self-assessment as part of the solution."
**Assessment finding:** absent.

**Work:** add `docs/furps.md` following the LP-0000 solution template's FURPS
structure: Functionality, Usability, Reliability, Performance, Supportability
, each scored honestly with evidence pointers (this repo's own docs + real
benchmark numbers once Item 10 lands).

**DoD:** document exists and is cross-referenced from the README.

**Effort:** ~1 h (final polish after benchmarks exist).

---

## Item 9: Benchmarks: CU costs + proof generation time

**Prize req:** "Document the compute unit (CU) cost of each on-chain operation
on LEZ devnet/testnet." + "Proof generation time and on-chain verification
compute unit benchmarks."
**Assessment finding:** no benchmarks anywhere.

**Work**
1. CU cost per instruction: instrument the standalone sequencer run to record
   compute units for `initialize_distribution`, `freeze_distribution`, and
   each privacy tx (mint, claim). Store the raw numbers in
   `docs/benchmarks/cu-costs.md` with the sequencer version + block context.
2. Proof generation time: time real `RISC0_DEV_MODE=0` proof generation for
   the claim path (and mint), record wall-clock on the benchmark machine,
   CPU/RAM, and toolchain version in `docs/benchmarks/proof-times.md`.
3. Add a small `examples/bench_cu.rs` (or a script) so the numbers are
   reproducible, and commit the recorded outputs.
4. Update FURPS Performance with these numbers.

**DoD:** numbers exist, were produced by real runs, and are reproducible via a
committed script. (Risk: CU accounting API may not expose per-instruction
numbers on the standalone sequencer: if so, record what IS exposed and say so,
rather than inventing units.)

**Effort:** ~2-4 h (after Item 10 makes real proofs runnable).

---

## Item 10: Real proofs: `RISC0_DEV_MODE=0` path

**Prize req:** "A reproducible end-to-end demo script is provided and works
against a real local sequencer with `RISC0_DEV_MODE=0`."
**Assessment finding:** demo defaults to `RISC0_DEV_MODE=1`; real proofs never
run in the current flow.

This is the **critical-path technical item**. The 7.2 GB machine previously
OOM'd the prover (the original `rx len failed` episode). Do this first, before
any evidence generation:

**Work**
1. Standalone proof probe exists at `/tmp/opencode/probe` (risc0 3.0.5) ,
   resurrect and extend it to time a single proof on the actual guest ELF.
2. Measure: can a claim proof complete on this machine? If not:
   - Option A: raise swap / use a bigger runner just for proof generation.
   - Option B: split proving from the wallet (remote/async prover service),
     documenting that proofs are generated off the demo host.
   - Option C: trim guest cycle count (the guest is already tiny; verify the
     `2^25` session limit headroom and record it).
3. Add a `RISC0_DEV_MODE`-aware gate: `demo.sh` must **fail loudly** if set to
   `1` when a `--require-real` flag is passed, and the documented prize flow
   uses `0`. Keep `1` only behind an explicit dev override.
4. Update `demo.sh` default to `RISC0_DEV_MODE=0` once the proof path is
   reliable, and record proof time in the demo output (feeds Item 9).

**DoD:** a full demo run completes with `RISC0_DEV_MODE=0` against the local
sequencer, and the terminal shows real proof generation. This is the gating
item for Items 9, 11, 12, 13.

**Effort:** 4-12 h (unpredictable: the OOM history is the risk).

---

## Item 11: Testnet deployments + evidence

**Prize req:** "At least 2 distinct distributions are deployed on LEZ testnet,
with a combined total of at least 20 unique claims completed across them; the
distributions must be reproducible and evidence must be provided." + "Program
deployed on LEZ testnet with a verified program ID."
**Assessment finding:** zero testnet presence.

**Work**
1. Obtain/configure testnet credentials and RPC endpoint (may require access
   we don't currently have: flag to the team if blocked).
2. Deploy the airdrop + token programs on testnet; record the verified program
   IDs and deployment tx hashes in `docs/testnet.md` and the README.
3. Run **distribution #1** (e.g., 10 recipients) and **distribution #2**
   (e.g., 10 different recipients) with all 20+ claims via the CLI.
4. Commit evidence: per-claim tx hashes, a claims log, and the deployment
   manifest: as JSON files under `evidence/`.
5. Make it reproducible: a `scripts/testnet_run.sh` that takes a recipient
   list and emits the same evidence structure.

**DoD:** ≥2 distribution accounts on testnet with verified program IDs, ≥20
unique claim tx hashes in the repo, reproducible via a committed script.

**Effort:** 4-8 h + network access. Highest external dependency.

---

## Item 12: GitHub issues for Logos tech problems

**Prize req:** "GitHub issues open for any problem encountered with Logos
technology."
**Assessment finding:** 0 issues.

**Work:** open issues on the relevant Logos repos (or this repo, if the
problems were local) for the genuinely-encountered upstream friction we hit ,
e.g., the OOM/`rx len failed` behavior, the `RISC0_DEV_MODE` verifier symmetry,
the "Commitment already seen" multi-mint failure mode, and the `NSSA_` vs
`LEE_` env-var mismatch. Link them from `docs/verification.md`.

**DoD:** N issues filed with clear reproductions, linked from the repo.

**Effort:** ~1 h.

---

## Item 13: Video demo

**Prize req:** "End-to-end demo video... builder narrates... demonstrates a
private claim from a shielded account... recording must show terminal output
(including proof generation) to confirm `RISC0_DEV_MODE=0`."
**Assessment finding:** no video.

**Work** (blocked on Item 10 for an honest `RISC0_DEV_MODE=0` walkthrough):
1. Record a narrated screen capture: architecture (2 min), then a live
   enroll→deploy→fund→claim→double-claim run against the local sequencer with
   `RISC0_DEV_MODE=0`, terminal visible.
2. Commit the video file (or a documented external link) in `evidence/`.
3. Ensure the recording shows the proof-generation step explicitly, since the
   prize calls that out.

**DoD:** a playable, narrated video exists and shows real proof output.

**Effort:** ~1-2 h (recording) + the Item 10 prerequisite.

---

## Item 14: Basecamp app GUI (and the phantom module)

**Prize req:** "Provide a Logos Basecamp app GUI with local build instructions,
downloadable assets, and loadable in Logos app (Basecamp)."
**Assessment finding:** `logos-airdrop-module/` is empty and uncommitted; the
README reference is a phantom.

**Decision point (must be resolved before starting):**
- Option A: **build the QML module** (the prize explicitly requires a GUI;
  this is a real P0). Scaffold a Logos Basecamp module (Rust provider + QML
  UI) wrapping the CLI/`airdrop-core` logic, with build instructions.
- Option B: **drop the claim** and remove the phantom README line. This keeps
  the repo honest but guarantees that requirement fails.

This plan assumes Option A (the requirement is explicit). If effort becomes
unsustainable, Option B is the fallback and must be paired with an honest note
in FURPS/README: never an empty directory.

**Work:** follow the Logos Basecamp module template (metadata, provider,
QML views for enroll/claim/status), add local build instructions and a
downloadable artifact path, and wire it into `README.md`.

**DoD:** a loadable Basecamp app module exists in the repo with build docs and
a populated (non-empty) `logos-airdrop-module/` directory.

**Effort:** 8-16 h. Second-biggest line item.

---

## Item 15: ZK eligibility proof: decide, then do one

**Prize req (scope section):** "ZK circuit(s) for eligibility and
claim-uniqueness proofs, targeting the Risc0 proving stack."
**Assessment finding:** the guest does not verify Merkle membership; the tree
is used off-chain only. The review calls this the biggest architectural risk.

**Decision point.** Two defensible resolutions; pick one and document it:
- **Option A: add a membership-proof claim instruction.** Recipients submit a
  claim tx containing a Merkle inclusion proof (for their `D_i` leaf) verified
  inside the guest against the committed root, plus a nullifier. This removes
  the "distributor mints eligibility" trust for *funding correctness* (not for
  recipient-privacy), aligns with the scope line, but grows the guest and risks
  the cycle budget. Effort: 8-16 h + proof-size work.
- **Option B: rigorous justification + tightened scope claim.** Argue (as
  `design.md` §2 does) that eligibility is enforced by the distributor's mint,
  that claims are native LEZ transfers, and that the *claim-uniqueness* proof
  is LEZ's nullifier set: and document why this satisfies the prize's
  *functionality* criteria while noting the scope-line deviation honestly.
  Effort: 2-3 h, but leaves the "biggest risk" partially open.

Recommendation: **attempt A**; if the guest cycle budget or proof size blocks
it, fall back to B **with an explicit deviation note**. Do not silently keep
the current position.

**DoD:** either a Merkle-inclusion claim instruction exists and passes a real
proof, or `docs/privacy-model.md` + `design.md` contain the precise,
non-evasive justification.

**Effort:** 2-16 h depending on the option.

---

## Phase ordering (execution sequence)

```
Phase 0  Repo hygiene       Items 1, 2, 7(layout fix only), 12
Phase 1  CI + tests         Items 3, 4
Phase 2  Guest errors       Item 5
Phase 3  REAL PROOFS        Item 10          <- critical path, go first risk-wise
Phase 4  Docs               Items 6, 7(full), 8 (draft), 9 (needs 10)
Phase 5  Evidence           Items 11, 13     <- need 10 + network access
Phase 6  GUI                Item 14
Phase 7  ZK decision        Item 15
Phase 8  Benchmarks + FURPS Items 9, 8(final)
Phase 9  Final cross-check  re-run assessment checklist; update
                           ASSESSMENT_ACKNOWLEDGEMENT.md verdict to reflect
                           only what is genuinely done
```

Rationale: Items 1-4 are cheap, unblock everything else, and establish that
CI is green before evidence is produced. Item 10 is scheduled first among the
hard items because the entire evidence story (9, 11, 13) depends on real
proofs. Item 6 (privacy model) is scheduled before evidence generation so the
threat model governs how evidence is collected and narrated. Item 14 is big
but independent, so it runs in parallel with any remaining work if needed.

---

## Definition of done (final checklist)

Cross-checked against LP-0003 success criteria:

- [ ] `LICENSE-MIT` + `LICENSE-APACHE-v2` + license fields (Item 1)
- [ ] `rust-toolchain.toml` committed; risc0 toolchain documented (Item 2)
- [ ] CI green on default branch (Item 3)
- [ ] Sequencer-backed integration test in CI (Item 4)
- [ ] Deterministic documented error codes (Item 5)
- [ ] Formal privacy model: threat model, distributor view, "unlinkable"
      defined, trusted-setup statement, residual leakage (Item 6)
- [ ] README + integration guide with real program addresses (Items 7, 11)
- [ ] FURPS self-assessment (Item 8)
- [ ] CU + proof-time benchmarks from real runs (Item 9)
- [ ] Demo runs with `RISC0_DEV_MODE=0` (Item 10)
- [ ] ≥2 testnet distributions, ≥20 unique claims, evidence committed
      (Item 11)
- [ ] GitHub issues filed for upstream friction (Item 12)
- [ ] Narrated video showing real proof output (Item 13)
- [ ] Loadable Basecamp GUI, non-empty module dir (Item 14)
- [ ] ZK eligibility: either implemented or precisely justified (Item 15)
- [ ] `ASSESSMENT_ACKNOWLEDGEMENT.md` updated to reflect only completed items

Honest note on completeness: items 1-8 are fully within our control and can be
completed to standard. Items 10-13 depend on either real-proof reliability on
this hardware or testnet access; if either is blocked, the submission still
cannot pass the hard gates regardless of how well everything else is done ,
that is why Item 10 is scheduled first.
