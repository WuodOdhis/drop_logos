//! SPEL macro wiring for the airdrop program.
//!
//! `DistributionState` lives here because the `#[account_type]` marker must be
//! at the same scope as `#[lez_program]` for the IDL scanner to find it. All
//! business logic is in `crate::handlers`; each `#[instruction]` body is a thin
//! delegate.

use airdrop_core::DistributionState as SharedDistributionState;
use borsh::{BorshDeserialize, BorshSerialize};
use spel_framework::prelude::*;

use crate::handlers;

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct DistributionState {
    pub root: [u8; 32],
    pub token_definition: [u8; 32],
    pub total_allocation: u128,
    pub num_eligible: u64,
    pub distributor: [u8; 32],
    pub committed_at: u64,
    pub active: u64,
}

impl DistributionState {
    pub fn is_active(&self) -> bool {
        self.active == 1
    }
}

// Compile-time drift guard: local `#[account_type]` struct must match the shared
// `airdrop_core::DistributionState` byte-for-byte.
const _: () = {
    assert!(
        core::mem::size_of::<DistributionState>()
            == core::mem::size_of::<SharedDistributionState>()
    );
};

#[cfg(test)]
mod layout_equivalence {
    use super::*;

    #[test]
    fn distribution_state_borsh_layout_matches_shared() {
        let local = DistributionState {
            root: [1u8; 32],
            token_definition: [2u8; 32],
            total_allocation: 3,
            num_eligible: 4,
            distributor: [5u8; 32],
            committed_at: 6,
            active: 7,
        };
        let shared = SharedDistributionState {
            root: [1u8; 32],
            token_definition: [2u8; 32],
            total_allocation: 3,
            num_eligible: 4,
            distributor: [5u8; 32],
            committed_at: 6,
            active: 7,
        };
        assert_eq!(
            borsh::to_vec(&local).unwrap(),
            borsh::to_vec(&shared).unwrap(),
            "DistributionState Borsh layout drifted from airdrop_core::DistributionState"
        );
    }
}

#[lez_program(instruction = "airdrop_core::Instruction")]
pub mod airdrop_program {
    #[allow(unused_imports)]
    use super::*;

    #[instruction]
    #[allow(clippy::too_many_arguments)] // SPEL-generated signature mirrors the on-chain tx inputs
    pub fn initialize_distribution(
        #[account(init, pda = [literal("distribution"), arg("distribution_id")])]
        distribution: AccountWithMetadata,
        #[account(signer)] distributor: AccountWithMetadata,
        clock_account: AccountWithMetadata,
        distribution_id: u64,
        root: [u8; 32],
        token_definition: [u8; 32],
        total_allocation: u128,
        num_eligible: u64,
    ) -> SpelResult {
        handlers::initialize_distribution(
            distribution,
            distributor,
            clock_account,
            distribution_id,
            root,
            token_definition,
            total_allocation,
            num_eligible,
        )
    }

    #[instruction]
    pub fn freeze_distribution(
        #[account(pda = [literal("distribution"), arg("distribution_id")])]
        distribution: AccountWithMetadata,
        #[account(signer)] distributor: AccountWithMetadata,
        distribution_id: u64,
    ) -> SpelResult {
        handlers::freeze_distribution(distribution, distributor, distribution_id)
    }
}
