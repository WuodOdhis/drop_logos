//! PDA derivation for the airdrop program.
//!
//! Every PDA address is `AccountId::for_public_pda(program_id, seed)` where the
//! seed is a 32-byte value. String labels are zero-padded to 32 bytes; `u64`
//! args are little-endian in the first 8 bytes; 32-byte args pass through.
//! Multiple seeds are combined via SHA-256(seed1 || seed2 || ...).

#[cfg(not(feature = "std"))]
extern crate alloc;

use sha2::{Digest, Sha256};

/// Zero-pad a string label to 32 bytes.
///
/// # Panics
///
/// Panics if the label is longer than 32 bytes.
pub fn label_seed(label: &str) -> [u8; 32] {
    let bytes = label.as_bytes();
    assert!(bytes.len() <= 32, "label '{label}' exceeds 32 bytes");
    let mut seed = [0u8; 32];
    seed[..bytes.len()].copy_from_slice(bytes);
    seed
}

/// Encode a `u64` as a 32-byte seed: little-endian in the first 8 bytes.
pub fn u64_seed(value: u64) -> [u8; 32] {
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&value.to_le_bytes());
    seed
}

/// Combine one or more 32-byte seeds into a single seed.
///
/// A single seed is returned as-is; multiple seeds are hashed via SHA-256.
///
/// # Panics
///
/// Panics if `seeds` is empty.
pub fn combine_seeds(seeds: &[&[u8; 32]]) -> [u8; 32] {
    assert!(!seeds.is_empty(), "at least one seed required");
    if seeds.len() == 1 {
        return *seeds[0];
    }
    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(*seed);
    }
    hasher.finalize().into()
}

/// Raw seed bytes for the distribution PDA: `[label("distribution"), u64(distribution_id)]`.
pub fn distribution_seed(distribution_id: u64) -> [u8; 32] {
    combine_seeds(&[&label_seed("distribution"), &u64_seed(distribution_id)])
}

/// Combined seed for use in chained-call `PdaSeed`s.
pub fn distribution_pda_seed(distribution_id: u64) -> [u8; 32] {
    distribution_seed(distribution_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_seed_zero_pads() {
        let seed = label_seed("distribution");
        assert_eq!(&seed[..12], b"distribution");
        assert_eq!(&seed[12..], &[0u8; 20]);
    }

    #[test]
    fn u64_seed_le() {
        let seed = u64_seed(0x0102_0304_0506_0708);
        assert_eq!(
            &seed[..8],
            &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
        assert_eq!(&seed[8..], &[0u8; 24]);
    }

    #[test]
    fn single_seed_passthrough() {
        let s = [7u8; 32];
        assert_eq!(combine_seeds(&[&s]), s);
    }

    #[test]
    fn multi_seed_deterministic() {
        let a = combine_seeds(&[&label_seed("distribution"), &u64_seed(1)]);
        let b = combine_seeds(&[&label_seed("distribution"), &u64_seed(1)]);
        assert_eq!(a, b);
        let c = combine_seeds(&[&label_seed("distribution"), &u64_seed(2)]);
        assert_ne!(a, c);
    }
}
