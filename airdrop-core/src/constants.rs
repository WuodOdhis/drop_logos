//! Constants shared by the airdrop guest and host.

/// Raw bytes of the CLOCK_50 system account ID, updated by the sequencer every
/// 50 blocks. Mirrored here so `airdrop-core` stays dependency-free.
pub const CLOCK_50_ACCOUNT_ID_BYTES: [u8; 32] = *b"/LEZ/ClockProgramAccount/0000050";

/// PDA seed label for the distribution config account.
pub const DISTRIBUTION_LABEL: &str = "distribution";

/// Default Sparse Merkle Tree depth for the hidden eligibility set.
pub const DEFAULT_TREE_DEPTH: usize = 32;

/// Zero commitment (invalid).
pub const ZERO_COMMITMENT: [u8; 32] = [0u8; 32];
