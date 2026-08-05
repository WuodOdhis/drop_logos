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
use spel_framework::prelude::{SpelError, SpelOutput, SpelResult};

use crate::program::DistributionState;

/// Deterministic, documented error codes returned by the airdrop program.
///
/// Codes are stable; do not renumber. See `docs/errors.md` for the full table
/// including conditions and client-side recovery actions. The codes below are
/// wrapped by SPEL as `Program error [code]: message` in the guest output and
/// surface in the host layer as `TransactionBuildError(ProgramProveFailed(..))`.
pub mod error_code {
    /// `distributor` account did not sign / is not authorized.
    pub const DISTRIBUTOR_NOT_AUTHORIZED: u32 = 1;
    /// Distribution account already holds committed data.
    pub const DISTRIBUTION_ALREADY_INITIALIZED: u32 = 2;
    /// `total_allocation` must be positive.
    pub const INVALID_TOTAL_ALLOCATION: u32 = 3;
    /// `num_eligible` must be positive.
    pub const INVALID_NUM_ELIGIBLE: u32 = 4;
    /// Eligibility root must be non-zero.
    pub const INVALID_ROOT: u32 = 5;
    /// The clock account is not the expected CLOCK_50 system account.
    pub const INVALID_CLOCK_ACCOUNT: u32 = 6;
    /// Caller is not the distributor that committed this distribution.
    pub const NOT_DISTRIBUTOR: u32 = 7;
    /// Freeze attempted on an already-frozen distribution.
    pub const DISTRIBUTION_ALREADY_FROZEN: u32 = 8;
}

/// `Distributor must sign`.
fn require_authorized(is_authorized: bool) -> Result<(), SpelError> {
    if is_authorized {
        Ok(())
    } else {
        Err(SpelError::custom(
            error_code::DISTRIBUTOR_NOT_AUTHORIZED,
            "Distributor must sign",
        ))
    }
}

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
fn require_clock(clock_account: &AccountWithMetadata) -> Result<u64, SpelError> {
    if *clock_account.account_id.value() == CLOCK_50_ACCOUNT_ID_BYTES {
        Ok(clock_core::ClockAccountData::from_bytes(clock_account.account.data.as_ref()).timestamp)
    } else {
        Err(SpelError::custom(
            error_code::INVALID_CLOCK_ACCOUNT,
            "Wrong clock account provided",
        ))
    }
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
) -> SpelResult {
    require_authorized(distributor.is_authorized)?;
    if !distribution.account.data.as_ref().is_empty() {
        return Err(SpelError::custom(
            error_code::DISTRIBUTION_ALREADY_INITIALIZED,
            "Distribution already initialized",
        ));
    }
    if total_allocation == 0 {
        return Err(SpelError::custom(
            error_code::INVALID_TOTAL_ALLOCATION,
            "Total allocation must be positive",
        ));
    }
    if num_eligible == 0 {
        return Err(SpelError::custom(
            error_code::INVALID_NUM_ELIGIBLE,
            "num_eligible must be positive",
        ));
    }
    if root == [0u8; 32] {
        return Err(SpelError::custom(
            error_code::INVALID_ROOT,
            "Root must be non-zero",
        ));
    }

    let committed_at = require_clock(&clock_account)?;

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
    Ok(SpelOutput::execute(states, vec![]))
}

/// Distributor freezes the distribution.
pub fn freeze_distribution(
    mut distribution: AccountWithMetadata,
    distributor: AccountWithMetadata,
    _distribution_id: u64,
) -> SpelResult {
    require_authorized(distributor.is_authorized)?;

    let mut state = read_distribution(&distribution);
    if state.distributor != *distributor.account_id.value() {
        return Err(SpelError::custom(
            error_code::NOT_DISTRIBUTOR,
            "Not the distributor",
        ));
    }
    if !state.is_active() {
        return Err(SpelError::custom(
            error_code::DISTRIBUTION_ALREADY_FROZEN,
            "Distribution already frozen",
        ));
    }

    state.active = 0;
    write_borsh(&mut distribution, &state);

    let states = vec![
        AccountPostState::new(distribution.account),
        AccountPostState::new(distributor.account),
    ];
    Ok(SpelOutput::execute(states, vec![]))
}

#[cfg(test)]
mod error_code_tests {
    use super::*;
    use nssa_core::account::{Account, AccountId};

    fn acct(is_authorized: bool) -> AccountWithMetadata {
        AccountWithMetadata {
            account: Account::default(),
            is_authorized,
            account_id: AccountId::new([0xAA; 32]),
        }
    }

    fn distributor_acct() -> AccountWithMetadata {
        let mut a = acct(true);
        a.account_id = AccountId::new([0xAB; 32]);
        a
    }

    fn clock_acct() -> AccountWithMetadata {
        let mut a = acct(false);
        a.account_id = AccountId::new(CLOCK_50_ACCOUNT_ID_BYTES);
        a.account.data = borsh::to_vec(&clock_core::ClockAccountData {
            block_id: 1,
            timestamp: 1,
        })
        .expect("encode clock data")
        .try_into()
        .expect("clock data fits");
        a
    }

    fn as_error(res: &SpelResult) -> u32 {
        match res.as_ref().unwrap_err() {
            SpelError::Custom { code, .. } => *code,
            other @ (SpelError::AccountCountMismatch { .. }
            | SpelError::InvalidAccountOwner { .. }
            | SpelError::AccountAlreadyInitialized { .. }
            | SpelError::AccountNotInitialized { .. }
            | SpelError::InsufficientBalance { .. }
            | SpelError::DeserializationError { .. }
            | SpelError::SerializationError { .. }
            | SpelError::Overflow { .. }
            | SpelError::Unauthorized { .. }
            | SpelError::PdaMismatch { .. }
            | SpelError::AccountOwnerMismatch { .. }) => {
                panic!("expected Custom error, got {other:?}")
            }
        }
    }

    #[test]
    fn initialize_rejects_unauthorized_distributor() {
        let res = initialize_distribution(
            acct(false),
            acct(false),
            clock_acct(),
            1,
            [1u8; 32],
            [2u8; 32],
            100,
            10,
        );
        assert_eq!(as_error(&res), error_code::DISTRIBUTOR_NOT_AUTHORIZED);
    }

    #[test]
    fn initialize_rejects_zero_total_allocation() {
        let res = initialize_distribution(
            acct(false),
            distributor_acct(),
            clock_acct(),
            1,
            [1u8; 32],
            [2u8; 32],
            0,
            10,
        );
        assert_eq!(as_error(&res), error_code::INVALID_TOTAL_ALLOCATION);
    }

    #[test]
    fn initialize_rejects_zero_num_eligible() {
        let res = initialize_distribution(
            acct(false),
            distributor_acct(),
            clock_acct(),
            1,
            [1u8; 32],
            [2u8; 32],
            100,
            0,
        );
        assert_eq!(as_error(&res), error_code::INVALID_NUM_ELIGIBLE);
    }

    #[test]
    fn initialize_rejects_zero_root() {
        let res = initialize_distribution(
            acct(false),
            distributor_acct(),
            clock_acct(),
            1,
            [0u8; 32],
            [2u8; 32],
            100,
            10,
        );
        assert_eq!(as_error(&res), error_code::INVALID_ROOT);
    }

    #[test]
    fn initialize_rejects_wrong_clock_account() {
        let res = initialize_distribution(
            acct(false),
            distributor_acct(),
            acct(false),
            1,
            [1u8; 32],
            [2u8; 32],
            100,
            10,
        );
        assert_eq!(as_error(&res), error_code::INVALID_CLOCK_ACCOUNT);
    }

    #[test]
    fn initialize_rejects_already_initialized() {
        // First initialize into a pre-committed account.
        let mut pre = acct(false);
        let state = DistributionState {
            root: [1u8; 32],
            token_definition: [2u8; 32],
            total_allocation: 100,
            num_eligible: 10,
            distributor: [0xAB; 32],
            committed_at: 1,
            active: 1,
        };
        write_borsh(&mut pre, &state);

        let res = initialize_distribution(
            pre,
            distributor_acct(),
            clock_acct(),
            1,
            [1u8; 32],
            [2u8; 32],
            100,
            10,
        );
        assert_eq!(as_error(&res), error_code::DISTRIBUTION_ALREADY_INITIALIZED);
    }

    #[test]
    fn initialize_succeeds_with_valid_inputs() {
        let res = initialize_distribution(
            acct(false),
            distributor_acct(),
            clock_acct(),
            1,
            [1u8; 32],
            [2u8; 32],
            100,
            10,
        );
        assert!(res.is_ok(), "valid initialize should succeed: {res:?}");
    }

    #[test]
    fn freeze_rejects_unauthorized_distributor() {
        let res = freeze_distribution(acct(false), acct(false), 1);
        assert_eq!(as_error(&res), error_code::DISTRIBUTOR_NOT_AUTHORIZED);
    }

    #[test]
    fn freeze_rejects_non_distributor_caller() {
        let mut pre = acct(false);
        let state = DistributionState {
            root: [1u8; 32],
            token_definition: [2u8; 32],
            total_allocation: 100,
            num_eligible: 10,
            distributor: [0xAB; 32],
            committed_at: 1,
            active: 1,
        };
        write_borsh(&mut pre, &state);

        // Authorized but wrong account id (not the committed distributor).
        let caller = acct(true);
        let res = freeze_distribution(pre, caller, 1);
        assert_eq!(as_error(&res), error_code::NOT_DISTRIBUTOR);
    }

    #[test]
    fn freeze_rejects_already_frozen() {
        let mut pre = acct(false);
        let state = DistributionState {
            root: [1u8; 32],
            token_definition: [2u8; 32],
            total_allocation: 100,
            num_eligible: 10,
            distributor: [0xAB; 32],
            committed_at: 1,
            active: 0,
        };
        write_borsh(&mut pre, &state);

        let res = freeze_distribution(pre, distributor_acct(), 1);
        assert_eq!(as_error(&res), error_code::DISTRIBUTION_ALREADY_FROZEN);
    }

    #[test]
    fn freeze_succeeds_for_distributor() {
        let mut pre = acct(false);
        let state = DistributionState {
            root: [1u8; 32],
            token_definition: [2u8; 32],
            total_allocation: 100,
            num_eligible: 10,
            distributor: [0xAB; 32],
            committed_at: 1,
            active: 1,
        };
        write_borsh(&mut pre, &state);

        let res = freeze_distribution(pre, distributor_acct(), 1);
        assert!(res.is_ok(), "valid freeze should succeed: {res:?}");
    }
}
