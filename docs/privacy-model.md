# Privacy Model / Threat Model — LP-0003 Private Airdrop

Status: formal privacy write-up for the LP-0003 submission. This document is
self-contained: an evaluator should be able to read it alone and answer every
"who learns what" question the prize lists. It formalizes and extends
`design.md` §7/§8; where this document contradicts the design doc, this
document is authoritative.

Every privacy claim here is defined **relative to a stated adversary**. The
word "unlinkable" never appears without a definition.

---

## 1. System model (what actually happens)

The system has three participant roles plus the platform:

- **Recipient** — holds a wallet; during enrollment generates two private
  accounts: `D_i` (the hidden allocation account) and `main_i` (their personal
  shielded claim-destination account). Writes an enrollment file
  (`Enrollment`, JSON) and hands it to the distributor **off-chain**.
- **Distributor** — collects enrollment files, commits an eligibility
  **merkle root** to chain, mints each `amount_i` into `D_i`, and can freeze.
- **Sequencer / chain** — the LEZ node that orders transactions, executes
  programs, maintains the nullifier set, and publishes all blocks.
- **Observer** — any party who reads the chain but participates in neither role.

The on-chain footprint is small and fixed:

1. **Deploy / commit.** The distributor deploys the airdrop + token programs
   and sends `InitializeDistribution { distribution_id, root,
   token_definition, total_allocation, num_eligible }`. The chain stores the
   `DistributionState` record (PDA `["distribution", distribution_id]`).
2. **Fund.** For each recipient, a privacy-preserving `Mint` of `amount_i`
   into `D_i` (recipient = `AccountIdentity::PrivateForeign`, identified by
   `D_i`'s public keys `(npk, vpk, identifier)` from the enrollment file).
3. **Claim.** For each recipient, a privacy-preserving `Transfer` from `D_i`
   to `main_i` (`send_transfer_transaction_private_owned_account`). LEZ
   publishes the spent note's nullifier; a second spend of `D_i` is rejected.
4. **Verify (off-chain).** `airdrop_status` re-computes the root from
   enrollment files and checks it equals the on-chain `root`.

Nothing else about recipients or amounts is written to chain.

---

## 2. Adversaries

### A1 — Chain observer

**Capabilities.** Full read access to all blocks, transactions, accounts,
program state, and the nullifier set, from genesis onward. Passive; does not
hold any wallet, enrollment file, or view key.

**Goal.** Recover which real-world recipient received what amount, or link a
recipient to a specific claim.

### A2 — Distributor

**Capabilities.** Everything an observer knows, **plus**: the full set of
enrollment files (name ↔ `D_i` ↔ `amount_i` ↔ `main_i` ↔ keys), its own mint
records (which `D_i` was funded with what), and the ability to freeze.

**Goal.** Determine who claimed what and when. (This adversary is a *trusted
party by design*; see §4.)

### A3 — Outside attacker

**Capabilities.** Network access (can send/reorder transactions, front-run),
and can act as a chain observer. Does **not** hold enrollment files or view
keys, and is not the distributor.

**Goals.** (a) Front-run a claim to steal or block it; (b) exclude a recipient
(censor their claim); (c) learn eligibility status of a victim.

### A4 — Other recipient

**Capabilities.** Owns their own wallet + enrollment file; acts as an observer
otherwise.

**Goal.** Learn the amounts or identities of other recipients.

---

## 3. Stage-by-stage "who learns what"

Legend: **O** = observer, **D** = distributor, **R** = recipients other than
the one acting, **—** = nothing.

| Stage | O learns | D learns | R learns |
|---|---|---|---|
| **Enroll** (off-chain) | nothing (no chain traffic) | the enrollment file: name, `amount_i`, `D_i`, `main_i`, `npk`, `vpk`, `identifier`, `leaf` | their own file only |
| **Deploy/commit** | program IDs; `DistributionState`: `root`, `token_definition`, `total_allocation`, `num_eligible`, `distributor`, `committed_at`, `active` | same as O (plus it signed the tx) | same as O |
| **Fund** (mint per `D_i`) | the existence and count of mint txs; the `D_i` account ids that received mints; shielded note ciphertexts (no amounts, no `main_i`, no recipient identity) | per-recipient `D_i → amount_i` (it chose them), plus all of O | same as O |
| **Claim** (transfer `D_i → main_i`) | the existence and timing of the transfer; the spent note's nullifier for `D_i`; shielded output ciphertexts (no amounts, no `main_i`, no identity) | per-recipient `D_i → main_i` and the claim timing (via enrollment data + nullifier set) | same as O |
| **Post-claim** | `DistributionState` unchanged; spent-nullifier set (count of claims); no further recipient data | every claim, mapped to a named recipient | same as O |

Key observable facts for **O**: the eligibility-set size (`num_eligible` is
public; so is the number of funded `D_i` and the number of claims), the timing
of each claim, and the root. Nothing ties a claim to a person *unless O
obtains enrollment data* (see §6 residual leakage).

---

## 4. Distributor trust statement (explicit)

**The distributor is a trusted party by design.** It:

- knows every `D_i` and the `name → D_i → amount_i → main_i` mapping (it holds
  the enrollment files),
- can link every funded `D_i` to its claim by watching the nullifier set it
  can already attribute to specific recipients,
- therefore learns **who claimed what and when** with certainty.

This is not a defect. LP-0003 scopes claim privacy to on-chain observers and
explicitly permits distributor knowledge as a trade-off axis. The distributor
is exactly as powerful as the party that selected the recipients and minted to
them — that is the point where identity information is *necessarily* revealed,
by design, to the one party that already holds it.

Consequences stated plainly:

- **The distributor cannot be deanonymized-from** (it is the source of the
  mapping), so the privacy guarantee is **not** distributor-hiding.
- A malicious distributor can mint to whomever it wants and can omit a
  recipient; `airdrop_status` detects a *root* mismatch but cannot force
  funding of every eligible recipient (design.md §7).
- The distributor holds the **viewing public keys** (`vpk`, `npk`) of `D_i` —
  public keys only; it cannot decrypt `D_i`'s notes without the viewing secret
  key, and does not need to in order to fund. It can, however, observe the
  nullifier activity of the accounts it already knows about.

---

## 5. Formal definitions

### Unlinkability (observer)

**Definition.** Relative to adversary **A1 (chain observer)**, a claim
(transfer `D_i → main_i`) is *unlinkable* if, given only the chain state and
its own knowledge (no enrollment files, no view keys, no off-chain side
channels), the observer's a-posteriori assignment of the claim to any
particular real-world recipient is no better than random over the eligible
set — i.e. the observer cannot determine which person claimed, which `D_i`
belongs to whom, nor the amount claimed.

**What satisfies it in this design.** The observer never sees `name`, never
sees a mapping `D_i → person`, and cannot decrypt the shielded note ciphertexts
(amounts and the `main_i` destination are hidden by LEZ's privacy-preserving
transaction format, which encrypts outputs to view keys the observer does not
have). The nullifier set proves *spent* but not *who spent*.

**Honest caveat.** Unlinkability is:

- *not* anonymity in a small set — the observer sees the timing and count of
  claims and the public `num_eligible`; if a distribution has one recipient,
  that recipient is trivially identified;
- *not* timing-safe — an observer correlating claim tx times with off-chain
  knowledge (e.g., who was told "claim now") can narrow candidates;
- *revocable if enrollment data leaks* — the moment an enrollment file reaches
  an observer, that observer knows `D_i`'s account id and `main_i`, and can
  retroactively attribute the corresponding claim (the distributor-knowledge
  asymmetry from §6).

### Eligibility-set privacy

**Definition.** Relative to **A1**, the eligibility set is *hidden* if the
observer cannot enumerate the recipients before claims occur.

**What satisfies it.** Only the merkle `root` is committed; leaves
(`H(D_i)`) are never published by the distributor. The observer learns the
*size* (`num_eligible`) but not the *members*.

### Claim-uniqueness (double-claim prevention)

Spending `D_i` publishes its note nullifier; LEZ's nullifier set rejects any
second spend of the same note. This is a platform-guaranteed property, not a
program property, and is asserted end-to-end in the integration tests
(`tests/sequencer_integration.rs`, `airdrop_claim` double-claim rejection).

### Amount privacy

**Definition.** Relative to **A1**, the amount of a mint or claim is *hidden*
if the observer cannot recover `amount_i` from chain data.

**What satisfies it.** Mint/transfer amounts live inside shielded, encrypted
note outputs that the observer cannot decrypt. `total_allocation` is public;
if all allocations are equal the observer infers the (equal) unit amount
from the public total and `num_eligible` — a residual leakage (see §6).

---

## 6. Residual leakage (honest enumeration)

Even with the strongest adversary definitions above, the following remain
visible to **A1**, and some to everyone:

1. **Set-size leakage.** `num_eligible`, the count of mint txs, and the
   count of claim nullifiers are all public. The eligibility-set size is
   revealed; so is how many recipients actually claimed.
2. **Amount leakage (equal-allocation case).** If allocations are uniform,
   `total_allocation / num_eligible` recovers the per-recipient amount.
3. **Timing correlation.** Claim times are public; an observer with
   off-chain knowledge (invitation times, recipient habits) can narrow
   identity candidates. No mix-net / delay layer is provided.
4. **The observer-can't-but-distributor-can asymmetry.** Unlinkability holds
   only against parties without enrollment data. The distributor holds it, so
   the "distributor-learns-everything" property in §4 is not leakage but a
   stated trust assumption. If enrollment files leak (compromise, poor
   handling), the affected recipients' claims become attributable by anyone
   holding the file — including retroactively.
5. **No anonymous channel for the claim itself.** The claim tx is broadcast
   from the recipient's IP; a network-level observer (or the sequencer) sees
   that a claim happened from a given endpoint. This is out of scope for
   on-chain privacy but is stated so it is not over-claimed.
6. **Program ID and PDA are public.** All participants and observers can see
   the airdrop program id, the distribution PDA, and its lifecycle
   (`active`/`committed_at`).

---

## 7. Trusted setup statement

**None.** Both cryptographic stacks used are **transparent**:

- **Poseidon hashing** (`rust-poseidon-bn254-pure`, rev `49e1042`) is a
  transparent hash with no trusted setup or ceremony.
- **risc0 zkVM 3.0.5** proof generation and verification use a transparent
  STARK/SNARK pipeline; there is no trusted setup requirement for either the
  guest program or the host proof path.

No auxiliary ceremony, keyset, or CRS is used anywhere in this system. The
only "trust" in the system is the distributor-role trust documented in §4 and
the LEZ platform itself (sequencer correct execution + nullifier enforcement).

---

## 8. Where identity information is revealed / withheld (summary)

| Point | Revealed | Withheld |
|---|---|---|
| Enrollment (off-chain) | identity to distributor | identity from chain |
| Root commit | set size, root | set members, names |
| Mint | `D_i` ids, mint count, ciphertexts | amounts, `main_i`, names |
| Claim | nullifier, timing | amount, destination, name |
| Post-claim | claim count, set size | per-claim attribution (to O) |

Identity information is *withheld from the chain and from other recipients at
every stage*, and is *revealed to the distributor at the enrollment stage by
design*.

---

## 9. Cross-reference with design.md

- `design.md` §7's claim "A chain observer sees shielded notes, not amounts or
  links between `D_i` and the recipient's other accounts" is refined here:
  it holds **relative to A1**, with the caveats in §5 (uniform-amount
  inference, timing) and §6.
- `design.md` §7's "Recipient privacy" and "Trust" bullets are formalized as
  §4 and §5 above; no contradiction remains once "unlinkable" is read as the
  observer-relative definition in §5.
- `design.md` §8 ("merkle proof verification out of scope", "no frontend")
  is unchanged; the eligibility-proof trade-off is tracked separately in the
  remediation plan (Item 15) and `design.md` §2.
