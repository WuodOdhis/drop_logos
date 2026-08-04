//! On-chain account derivations for the airdrop program.
//!
//! Mirrors the SPEL `pda = [literal("distribution"), arg("distribution_id")]`
//! declaration: the PDA is `AccountId::for_public_pda(program_id, seed)` with
//! `seed = SHA-256("distribution" (zero-padded) || u64_le(distribution_id))`,
//! computed by `airdrop_core::distribution_seed`.

use nssa::AccountId;
use nssa_core::program::{PdaSeed, ProgramId};

/// Distribution config account: `seeds = [literal("distribution"), arg("distribution_id")]`.
pub fn distribution_account_id(program_id: &ProgramId, distribution_id: u64) -> AccountId {
    let seed = airdrop_core::distribution_seed(distribution_id);
    AccountId::for_public_pda(program_id, &PdaSeed::new(seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_program_id() -> ProgramId {
        bytemuck::cast([1u8; 32])
    }

    #[test]
    fn distribution_pda_is_deterministic() {
        let p = mock_program_id();
        assert_eq!(
            distribution_account_id(&p, 7),
            distribution_account_id(&p, 7)
        );
        assert_ne!(
            distribution_account_id(&p, 7),
            distribution_account_id(&p, 8)
        );
    }

    #[test]
    fn distribution_pda_matches_guest_seed() {
        let p = mock_program_id();
        let seed = airdrop_core::distribution_seed(7);
        let from_seed = AccountId::for_public_pda(&p, &PdaSeed::new(seed));
        assert_eq!(from_seed, distribution_account_id(&p, 7));
    }
}
