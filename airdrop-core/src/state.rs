//! Shared Borsh-encoded state for the airdrop program.
//!
//! This mirrors the `#[account_type]` definition in
//! `methods/guest/src/bin/airdrop.rs`. Field declaration order is the byte
//! layout (Borsh encodes fixed-width primitives + fixed arrays in declaration
//! order, no length prefixes). New fields are only ever APPENDED so existing
//! offsets stay stable.

use borsh::{BorshDeserialize, BorshSerialize};

/// On-chain distribution configuration.
///
/// Stored in the `["distribution", distribution_id]` PDA. Borsh-encoded
/// size: 136 bytes (fixed-width primitives + fixed arrays, no length prefixes).
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct DistributionState {
    /// Merkle root of the hidden eligibility set (commitments).
    pub root: [u8; 32],
    /// Account ID of the airdrop token definition.
    pub token_definition: [u8; 32],
    /// Total allocation being distributed (public, per prize spec).
    pub total_allocation: u128,
    /// Number of eligible recipients (public, per prize spec).
    pub num_eligible: u64,
    /// Distributor account (authority for freeze).
    pub distributor: [u8; 32],
    /// Timestamp of the on-chain commitment.
    pub committed_at: u64,
    /// 1 while claims/funding are allowed, 0 once frozen.
    pub active: u64,
}

impl DistributionState {
    /// Borsh-encoded size of [`DistributionState`].
    pub const SIZE: usize = 32 + 32 + 16 + 8 + 32 + 8 + 8;

    pub fn is_active(&self) -> bool {
        self.active != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borsh_size_matches_const() {
        let state = DistributionState {
            root: [1u8; 32],
            token_definition: [2u8; 32],
            total_allocation: 3,
            num_eligible: 4,
            distributor: [5u8; 32],
            committed_at: 6,
            active: 7,
        };
        assert_eq!(
            borsh::to_vec(&state).unwrap().len(),
            DistributionState::SIZE,
            "Borsh encoding drifted from DistributionState::SIZE"
        );
    }
}
