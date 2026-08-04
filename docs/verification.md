# Verification notes

How every API this workspace touches was verified against the pinned sources,
so a fresh build is not at the mercy of upstream drift.

## Pins

| Dependency | Pin |
|---|---|
| `lee` / `lee_core` (aliases `nssa` / `nssa_core`) | git `logos-blockchain/logos-execution-zone`, tag `v0.2.0-rc6` |
| `wallet`, `common`, `sequencer_service_rpc`, `programs`, `token_core`, `clock_core` | same tag `v0.2.0-rc6` |
| `spel-framework` | git `0x-r4bbit/spel`, rev `91023c9115bf88173b0d25d2e905f2a55ef0313b` |
| `risc0-zkvm` | `3.0.5` (guest toolchain `r0.1.88.0`) |
| `rust-poseidon-bn254-pure` | rev `49e1042` |
| Rust toolchain | `1.94.0` (`rust-toolchain.toml`) |

## Guest-side (SPEL macro semantics)

- `#[account(init, pda = [literal("distribution"), arg("distribution_id")])]`
  — `literal` is accepted as an alias for `const`; `arg` derives the seed from
  the instruction argument (`spel-framework-macros/src/lib.rs`).
- PDA seed combining is `SHA-256(seed1 || seed2 || ...)`; strings are
  zero-padded to 32 bytes; `u64` is little-endian in the first 8 bytes
  (`spel-framework-core/src/pda.rs`, `compute_pda` / `ToSeed`). This is mirrored
  by `airdrop-core::{label_seed, u64_seed, combine_seeds}`.
- `SpelOutput::execute(accounts, calls)` is the builder for instruction
  post-states (`spel-framework-core/src/spel_output.rs`). `#[account_type]`
  structs must live at the same scope as `#[lez_program]` for the IDL scanner.
- `AccountPostState::new_claimed_if_default(account, Claim::Pda(PdaSeed::new(seed)))`
  claims the init PDA (`lee/state_machine/core/src/program.rs`).

## Wallet / token program

- `Token::send_new_definition_private_owned_definiton_and_supply(def, supply,
  name, total_supply)` — creates a private fungible definition + supply
  (`lez/wallet/src/program_facades/token.rs:95`).
- `Token::send_mint_transaction_private_foreign_account(def, npk, vpk,
  identifier, amount)` — `Mint` to `AccountIdentity::PrivateForeign`
  (`token.rs:498`).
- `Token::send_transfer_transaction_private_owned_account(from, to, amount)` —
  `Transfer` between two owned accounts (`token.rs:149`).
- Wallet decodes/inserts private outputs automatically
  (`sync_private_accounts_with_tx` → `decode_insert_privacy_preserving_transaction_results`
  → `insert_private_account`, `lez/wallet/src/lib.rs`).
- Token holdings live in `account.data` as `TokenHolding::Fungible { definition_id, balance }`
  (`lez/programs/token/core/src/lib.rs`), decodable via `TryFrom<&Data>`.
- `NullifierPublicKey(pub [u8; 32])` (`lee/state_machine/core/src/nullifier.rs`);
  `ViewingPublicKey = MlKem768EncapsulationKey` (1184 bytes),
  `from_bytes(Vec<u8>)` / `to_bytes() -> &[u8]`
  (`lee/state_machine/core/src/encryption/shared_key_derivation.rs`).
- CLOCK_50 id is `*b"/LEZ/ClockProgramAccount/0000050"`
  (`lez/programs/clock/core/src/lib.rs`), decoded with
  `clock_core::ClockAccountData::from_bytes(data).timestamp`.

## Host transaction plumbing

All of `Message::try_new`, `WitnessSet::for_message`, `PublicTransaction::new`,
`ProgramDeploymentTransaction::new`, `get_account_public(_signing_key)`,
`get_accounts_nonces`, `sequencer_client.send_transaction` follow the working
`lez-rln/src/rln/client.rs` patterns from the scaffold.

## Toolchain

- `risc0-build 3.0.5`'s `DEFAULT_DOCKER_TAG` is `r0.1.88.0`; the local (non-docker)
  build uses the toolchain installed via `rzup` (RustToolchain component →
  `risc0/rust` tag `r0.1.88.0`, symlinked as rustup toolchain `risc0`).
- `RUSTC_BOOTSTRAP=1` is needed on the host because `rust-poseidon-bn254-pure`
  declares `#![feature(generic_const_exprs, bigint_helper_methods)]`, which the
  stable 1.94.0 host rustc rejects otherwise.
