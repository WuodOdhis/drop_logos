//! Shared serde `Instruction` enum for the airdrop program.
//!
//! Defined once and consumed by both the guest (via the SPEL macro arg
//! `#[lez_program(instruction = "airdrop_core::Instruction")]`) and the host
//! when building transactions. Variants and field order must match the
//! `#[instruction]` fn parameter lists in `methods/guest/src/bin/airdrop.rs`
//! (account params stripped, remaining args preserved in order).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Instruction {
    /// Distributor commits to the hidden eligibility set.
    ///
    /// Accounts: `[distribution]` (init PDA), `[distributor]` (signer).
    InitializeDistribution {
        distribution_id: u64,
        root: [u8; 32],
        token_definition: [u8; 32],
        total_allocation: u128,
        num_eligible: u64,
    },
    /// Distributor freezes the distribution (no further funding).
    ///
    /// Accounts: `[distribution]` (mut PDA), `[distributor]` (signer).
    FreezeDistribution {
        distribution_id: u64,
    },
}
