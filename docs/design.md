# Private Airdrop on LEZ: Design

Prize LP-0003 (airdrop/allowlist). This document records the design decisions,
the exact mechanism used, and the assumptions that shape the implementation.

## 1. Problem

A distributor wants to give tokens to a hidden set of recipients:

1. The recipient set must be **hidden** until claims happen (no public list).
2. Each recipient must be able to claim **privately** (nobody learns who
   claimed what, or whether a given address claimed).
3. Each recipient can claim **at most once** (double-claim prevention).
4. Recipients get **shielded receipts** (provable claim, no public link).

## 2. Chosen mechanism

The airdrop program commits to the eligibility set on chain via a **merkle
root** over `H(D_i)` for each recipient, where `D_i` is a private account the
recipient generated during enrollment. The program stores this root: plus the
token definition, totals, distributor authority, and commit timestamp: in a
PDA `["distribution", distribution_id]`.

**Claiming does not touch the airdrop program.** Instead:

- `airdrop_fund` (distributor): mints `amount_i` into `D_i` using the token
  program's `Mint` with a **private foreign account** recipient
  (`send_mint_transaction_private_foreign_account`). The distributor needs no
  secret key for `D_i`: only `(npk, vpk, identifier)`: which is exactly what
  the recipient's enrollment file publishes.
- `airdrop_claim` (recipient): spends `D_i` with the token program's
  `Transfer` between two **private owned accounts**
  (`send_transfer_transaction_private_owned_account`), moving the balance into
  the recipient's own shielded account. LEZ's native nullifier set makes a
  second spend of `D_i` impossible.

This design reuses the protocol's privacy machinery (shielded transaction
format, view-key decryption, nullifier checking) instead of re-implementing a
private transfer inside the airdrop program. Consequences:

- The guest program is tiny (two instructions, no proof verification), so it
  compiles to a small ELF and stays far under the sequencer's session/cycle and
  per-tx size limits.
- The merkle root is an on-chain **commitment to the eligibility set**, and the
  `airdrop_status` bin re-verifies it from the enrollment files: but the
  program itself does not gate claims on merkle proofs. Eligibility is enforced
  by the distributor's mint (only enrolled recipients get funded), and
  double-claim is enforced by the nullifier set.

## 3. Data model

### `DistributionState` (216 bytes, PDA `["distribution", distribution_id]`)

```
root:            [u8; 32]   Poseidon root of the hidden eligibility set
token_definition: [u8; 32]   account id of the airdrop token definition
total_allocation: u128       total tokens to distribute (public)
num_eligible:     u64        number of eligible recipients (public)
distributor:      [u8; 32]   authority that can freeze
committed_at:     u64        CLOCK_50 timestamp of the commitment
active:           u64        1 while claims/funding allowed, 0 when frozen
```

### Instruction set

```rust
enum Instruction {
    Initialize { distribution_id: u64, root: [u8; 32], token_definition: [u8; 32],
                 total_allocation: u128, num_eligible: u64 },
    Freeze      { distribution_id: u64 },
}
```

`Initialize` claims the distribution PDA, requires the distributor signature,
records `committed_at` from the CLOCK_50 account, and refuses to re-initialize
(data must be empty). `Freeze` flips `active = 0` (distributor-only).

### Enrollment file (`Enrollment`, JSON, exchanged recipient → distributor)

```
name           human label (demo only)
amount         allocation in token units
d_account_id   D_i (public account id, hex)
main_account_id the recipient's personal shielded account (hex)
npk            D_i nullifier public key (hex, 32B)
vpk            D_i viewing public key (hex, 1184B, ML-KEM-768 encapsulation key)
identifier     D_i private-account identifier (u128)
leaf           H(D_i), the eligibility merkle leaf (hex, 32B)
```

### Run manifest (`RunManifest`, JSON, distributor → recipients)

Distributor program id, distribution account, token definition, supply account,
distributor account, committed root, totals.

## 4. PDA derivation

The guest declares the distribution account as:

```rust
#[account(init, pda = [literal("distribution"), arg("distribution_id")])]
distribution: AccountWithMetadata,
```

SPEL derives the PDA as `AccountId::for_public_pda(program_id, PdaSeed::new(seed))`
where `seed = SHA-256("distribution" zero-padded || u64_le(distribution_id))`
(`spel-framework-core/src/pda.rs`). The host mirrors this exactly in
`airdrop-core::distribution_seed`: both sides are pinned to the same SHA-256
combining rule, so host-computed and guest-claimed PDAs match.

## 5. Eligibility merkle tree

- Depth 32 (`DEFAULT_TREE_DEPTH`), Poseidon-BN254 (`rust-poseidon-bn254-pure`
  at rev `49e1042`: the same crate/rev the LEZ guest pins, so host and guest
  hashing are bit-identical).
- Leaf `i` = `H(D_i)`; the root is committed on chain.
- `SparseMerkleTree` stores only modified nodes; unmodified subtrees use cached
  default hashes (`compute_default_hashes`).

## 6. Funding / claiming mechanics (verified against wallet + token sources)

- **Fund**: `Token::send_mint_transaction_private_foreign_account(def, npk,
  vpk, identifier, amount)`: a privacy-preserving `Mint` whose recipient is
  `AccountIdentity::PrivateForeign`. The definition account is a wallet private
  account (minting is authorized because the definition account is authorized),
  so no per-recipient distributor key is needed. The distributor's wallet can't
  decrypt `D_i`, and doesn't need to.
- **Claim**: `Token::send_transfer_transaction_private_owned_account(D_i,
  main, amount)`: a privacy-preserving `Transfer` between two owned accounts.
  The wallet decrypts both outputs after the block seals
  (`sync_private_accounts_with_tx` → `decode_insert_privacy_preserving_transaction_results`).
- **Double claim**: spending `D_i` publishes its nullifier; a second spend of
  the same note is rejected by the sequencer with a nullifier/spent error
  (asserted in `airdrop_claim`).

## 7. Security model / assumptions

> Privacy terminology ("unlinkable", "hidden set", "distributor view") is
> defined formally, per adversary, in `docs/privacy-model.md`. That document
> is authoritative for any privacy claim; this section is the short form.

- **Hidden set**: the root is committed; leaves are never published by the
  distributor. The enrollment files are held by the distributor and the
  recipients.
- **Recipient privacy**: minting and claiming are privacy-preserving
  transactions. A chain observer sees shielded notes, not amounts or links
  between `D_i` and the recipient's other accounts (unless the recipient links
  them off-chain).
- **Trust**: the distributor mints to the committed set. `airdrop_status`
  verifies the on-chain root against the enrollment files but cannot force the
  distributor to fund every eligible recipient (that would require either a
  per-recipient claim-with-proof instruction in the program, or a public
  funding list: both intentionally out of scope for this prize submission).
- **Freeze** is a kill-switch for the distribution, not a permission system for
  claims.

## 8. What is intentionally out of scope

- Merkle-proof verification inside the guest program (see §2).
- Recovery of unclaimed funds, per-recipient caps beyond the mint, or
  non-fungible airdrops.
- A frontend beyond the CLI bins and the QML module scaffold.
