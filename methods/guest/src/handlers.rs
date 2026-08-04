//! Pure-logic implementations of each airdrop instruction.
//!
//! Each function takes typed `AccountWithMetadata` inputs + instruction args,
//! performs all parsing/validation/computation, and returns the post-states the
//! SPEL macro handler wraps. Keeping the logic out of the macro-processed module
//! makes it directly callable from unit tests without going through the zkVM.

use airdrop_core::{CLOCK_50_ACCOUNT_ID_BYTES, distribution_pda_seed};
use borsh::{BorshDeserialize, BorshSerialize};
use nssa_core::{
    account::AccountWithMetadata,
    program::{AccountPostState, Claim, PdaSeed},
};
use spel_framework::prelude::SpelOutput;

use crate::program::DistributionState;

fn write_borsh<T: BorshSerialize>(account: &mut AccountWithMetadata, value: &T) {
    account.account.data = borsh::to_vec(value)
        .expect("borsh serialize DistributionState")
        .try_into()
        .expect("DistributionState fits in account.data");
}

/// Read the current `DistributionState` from the distribution account.
fn read_distribution(account: &AccountWithMetadata) -> DistributionState {
    DistributionState::try_from_slice(account.account.data.as_ref())
        .expect("decode DistributionState")
}

/// Validate that `clock_account` is the expected CLOCK_50 system account and
/// return its current unix timestamp.
fn require_clock(clock_account: &AccountWithMetadata) -> u64 {
    assert!(
        *clock_account.account_id.value() == CLOCK_50_ACCOUNT_ID_BYTES,
        "Wrong clock account provided"
    );
    clock_core::ClockAccountData::from_bytes(clock_account.account.data.as_ref()).timestamp
}

/// Distributor commits to the hidden eligibility set.
pub fn initialize_distribution(
    mut distribution: AccountWithMetadata,
    distributor: AccountWithMetadata,
    clock_account: AccountWithMetadata,
    distribution_id: u64,
    root: [u8; 32],
    token_definition: [u8; 32],
    total_allocation: u128,
    num_eligible: u64,
) -> SpelOutput {
    assert!(distributor.is_authorized, "Distributor must sign");
    assert!(
        distribution.account.data.as_ref().is_empty(),
        "Distribution already initialized"
    );
    assert!(total_allocation > 0, "Total allocation must be positive");
    assert!(num_eligible > 0, "num_eligible must be positive");
    assert_ne!(root, [0u8; 32], "Root must be non-zero");

    let committed_at = require_clock(&clock_account);

    let state = DistributionState {
        root,
        token_definition,
        total_allocation,
        num_eligible,
        distributor: *distributor.account_id.value(),
        committed_at,
        active: 1,
    };
    write_borsh(&mut distribution, &state);

    let seed = distribution_pda_seed(distribution_id);
    let states = vec![
        AccountPostState::new_claimed_if_default(
            distribution.account,
            Claim::Pda(PdaSeed::new(seed)),
        ),
        AccountPostState::new(distributor.account),
        AccountPostState::new(clock_account.account),
    ];
    SpelOutput::execute(states, vec![])
}

/// Distributor freezes the distribution.
pub fn freeze_distribution(
    mut distribution: AccountWithMetadata,
    distributor: AccountWithMetadata,
    _distribution_id: u64,
) -> SpelOutput {
    assert!(distributor.is_authorized, "Distributor must sign");

    let mut state = read_distribution(&distribution);
    assert_eq!(
        state.distributor,
        *distributor.account_id.value(),
        "Not the distributor"
    );
    assert!(state.is_active(), "Distribution already frozen");

    state.active = 0;
    write_borsh(&mut distribution, &state);

    let states = vec![
        AccountPostState::new(distribution.account),
        AccountPostState::new(distributor.account),
    ];
    SpelOutput::execute(states, vec![])
}
