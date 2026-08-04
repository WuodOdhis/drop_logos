# Post-Assessment Acknowledgment

Status: **VERDICT ACCEPTED — DOES NOT PASS**

This document is written *after* an external review of this repository
(`https://github.com/WuodOdhis/drop_logos`) that scored the submission at
roughly 16/48 and recommended a **DOES NOT PASS** verdict.

The purpose of this file is not to appeal the verdict. It is to record, with
intellectual honesty and without self-serving framing, exactly which findings
are true, how they were verified against the actual repository state, and
where the assessment's *details* are imprecise — because a rigorous
acknowledgment must neither inflate nor deflate the truth. Every claim below
was re-checked against the committed tree, not against memory.

---

## 1. Overall verdict

**The assessment is correct in substance. The submission fails the hard
requirements.** The categories that are genuinely solid (contract + ZK core,
CLI tooling) do not outweigh the missing hard gates (CI, LICENSE, testnet
deployments, 20-claim evidence, video, GUI, FURPS, benchmarks, formal privacy
model). The architecture is competent; the *submission* is incomplete. Both
statements are true at the same time.

---

## 2. Findings CONFIRMED as true (with evidence)

Each row was re-verified in the committed tree (commit `f3ea0ff`).

| # | Assessment finding | Verification result |
|---|---|---|
| P0 | No CI configuration | Confirmed. No `.github/` directory exists anywhere in the repo. No workflow files. |
| P0 | No LICENSE file | Confirmed. No `LICENSE*` at repo root. `airdrop-core/Cargo.toml:6` declares `license = "MIT OR Apache-2.0"`, but the root `Cargo.toml` has **no** `license` field and no license text exists. |
| P0 | No testnet deployments | Confirmed. No program IDs, tx hashes, deployment logs, or network references anywhere. All development ran against a **local** dev sequencer. |
| P0 | No ≥20-claim evidence | Confirmed. The demo runs exactly 3 recipients (`alice,bob,carol`) with no captured evidence artifacts. |
| P0 | No video demo | Confirmed. No `.mp4`/`.webm` or any video file; no external link. |
| P0 | No Basecamp GUI | Confirmed for the *pushed repo*. The README (`README.md:50`) lists `logos-airdrop-module/` as a "Basecamp QML module (Rust provider)", but the directory is **empty** and **not committed** (`git ls-files logos-airdrop-module` → nothing). From the reviewer's perspective the reference is a phantom. |
| P0 | No FURPS self-assessment | Confirmed. The string "FURPS" appears nowhere in the repo. |
| P0 | No CU / proof-gen benchmarks | Confirmed. No `criterion`, `#[bench]`, or `benches/` anywhere. No compute-unit numbers. |
| P0 | Demo defaults to `RISC0_DEV_MODE=1` | Confirmed. `scripts/demo.sh:28`: `export RISC0_DEV_MODE="${RISC0_DEV_MODE:-1}"`. The demo therefore produces *mock* receipts by default, not real succinct proofs. |
| P1 | No formal threat model | Confirmed. `docs/design.md` §7 ("Security model / assumptions") is a short bullet list, not a formal threat model: no adversary definitions, no capability assumptions, no success criteria. |
| P1 | Distributor-side leakage not addressed | Confirmed. `design.md` §7 states the enrollment files are "held by the distributor and the recipients" and that the distributor "mints to the committed set", but never analyzes that the distributor can link every `D_i` to a recipient and observe every claim. This is the largest privacy blind spot and it is unstated. |
| P1 | "Unlinkable" never defined | Confirmed. The string "unlink" (case-insensitive) does not appear in `docs/design.md`, `docs/verification.md`, or `README.md`. No formal definition exists. |
| P1 | No deterministic error codes | Confirmed. `methods/guest/src/handlers.rs` uses `assert!` with human strings (e.g. `:52` "Distributor must sign", `:53`, `:57`, `:58`, `:92`, `:100`) and `.expect()` (`:20,:22,:28`). There is **no** error enum in the guest. The requirement for "deterministic, documented error codes" is unmet. |
| P2 | No ZK eligibility proof | Confirmed, and acknowledged as deliberate. `docs/design.md` §2 explicitly defers eligibility to "the distributor minting only enrolled recipients" and lists merkle-proof verification in the guest as out of scope (§8). The Merkle tree is used only off-chain (`airdrop_status`), never inside a proof. The review's "architectural gap" characterization is accurate. |
| P2 | `rust-toolchain.toml` missing from repo | Confirmed. No `rust-toolchain*` file is committed, yet `docs/verification.md` lists the pin as "`1.94.0` (`rust-toolchain.toml`)" — the doc references a file that is not in the repo. |
| P2 | Tests do not run against a sequencer | Confirmed. `tests/airdrop_integration.rs:1-6` states they "run without the zkVM or a sequencer." No sequencer-backed integration test exists. |
| — | No GitHub issues filed | Confirmed. `gh issue list --repo WuodOdhis/drop_logos` returns zero issues. |

---

## 3. Findings confirmed with nuance (agreed, but the reason matters)

These are true failures, but the *mechanism* matters for the fix, so they are
stated precisely rather than as flat admissions.

- **"No ZK circuit for eligibility."** True against the literal prize
  requirement, and it was a *conscious* design decision documented in
  `design.md` §2: the guest program deliberately does not verify Merkle
  membership, instead relying on LEZ's native privacy machinery. The review is
  right that this fails the stated requirement; the intent does not excuse the
  gap, but the fix is either (a) a membership-proof instruction in the guest,
  or (b) a much more rigorous written justification — not a coding slip.

- **"The distributor can link claims."** True, and it is the most damaging
  privacy finding. It is also fundamental to the current design: the
  distributor authors the enrollment files and the mints, so it holds the
  `D_i`↔recipient mapping by construction. No remediation can be "document it
  away"; the design itself vests this trust in the distributor. The write-up
  must state this as an explicit trust assumption, or the design must change.

- **"Failed claims do not mark claimant as claimed."** The review marks this
  Pass, and the verification agrees (a rejected transfer publishes no
  nullifier and writes no state). This one is a genuine strength, not a
  failure.

---

## 4. Where the assessment is imprecise (honesty cuts both ways)

These do **not** change the verdict, but a rigorous acknowledgment records
them so that we neither over- nor under-concede.

- **Merkle test count.** The review says "8 unit tests" for the merkle
  implementation. The committed `airdrop-core/src/merkle.rs` contains **7**
  `#[test]` functions; the whole `airdrop-core` crate contains 12. This is a
  minor numeric imprecision, not a substantive disagreement.

- **`logos-airdrop-module` "does not exist."** More precisely: the directory
  *exists locally as an empty scaffold* but contains zero files and is not in
  the committed tree. The reviewer's operative claim — that the README's
  module reference is absent from the deliverable — is correct.

- **"0 issues on the repo."** Confirmed as factually true at review time; this
  is noted here only because it is an operational gap we control and can fix,
  not a fact about the code.

- **The review's Pass on items 1–5 and 11–15.** Agreed. The core LEZ program
  (Poseidon root commitment, private claim flow, nullifier double-claim
  prevention) and the CLI/IDL tooling are correctly assessed as working. The
  "What Works Well" section is fair and not flattering.

---

## 5. Root-cause reflection (why these gaps exist)

Honest causes, in order of weight — none of these are excuses, but naming them
is what makes the next plan realistic:

1. **Effort went to the hard technical core first.** The contracting, Merkle
   tree, guest program, CLI tools, and a *working end-to-end demo* (including
   real double-claim rejection) were built and verified against pinned
   upstream sources. Operational/packaging work was treated as "later."

2. **No reviewer-facing evidence pipeline.** Because everything ran against a
   local dev sequencer with mock proofs (`RISC0_DEV_MODE=1`), there was never
   a natural artifact trail (testnet program IDs, claim tx hashes, receipts).
   Building the evidence is therefore not just "running the demo" — it requires
   a proof-enabled path that was not exercised.

3. **Documentation was written to explain the design, not to satisfy a
   rubric.** `design.md` and `verification.md` are technical and honest about
   the architecture, but were never cross-checked against the prize's
   documentation checklist (threat model, unlinkability definition, leakage
   analysis, trusted-setup statement, benchmarks, FURPS).

4. **The GUI and module scaffold were left as a stub.** `logos-airdrop-module`
   is an empty directory; `integration_tests/` is likewise empty and
   uncommitted. These were started and abandoned, not deliberately excluded.

5. **No CI and no license were added before the push.** Both are trivially
   fixable and were simply overlooked in the final packaging step.

---

## 6. What this document does not do

- It does **not** propose the remediation plan. Fixing comes next, after this
  acknowledgment is committed.
- It does **not** re-litigate the scorecard math. The categorical verdict —
  DOES NOT PASS — is accepted.
- It does **not** make promises. Where this document says "we can fix X," that
  is a statement of feasibility, not a commitment with a date.

---

## 7. Sign-off on truthfulness

To the best of the author's knowledge, every "Confirmed" claim in §2 and every
"nuance" claim in §3 was re-verified against the committed tree at the time of
writing. Where the evidence contradicts a review detail (§4), it is recorded
with the same weight as the concessions, because an acknowledgment that only
concedes is not rigorous — it is flattery in the other direction.
