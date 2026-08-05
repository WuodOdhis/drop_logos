//! Poseidon Sparse Merkle Tree over the hidden eligibility set.
//!
//! The host builds this tree from enrollment commitments and commits the root
//! on-chain via `initialize_distribution`. The same Poseidon permutation used
//! by the LEZ guest (`rust-poseidon-bn254-pure`) keeps host and guest hashing
//! bit-identical.

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, vec, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use rust_poseidon_bn254_pure::{
    bn254::field::Felt,
    poseidon::permutation::{compress_1, compress_2},
};

use crate::constants::DEFAULT_TREE_DEPTH;

pub const ZERO: [u8; 32] = [0u8; 32];

/// BN254 base field prime, little-endian 64-bit limbs:
/// `0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47`.
const BN254_PRIME_LIMBS: [u64; 4] = [
    0x3c20_8c16_d87c_fd47,
    0x9781_6a91_6871_ca8d,
    0xb850_45b6_8181_585d,
    0x3064_4e72_e131_a029,
];

/// Little-endian u64 limbs of a 32-byte value.
fn to_u64_limbs(input: &[u8; 32]) -> [u64; 4] {
    let mut limbs = [0u64; 4];
    for (i, limb) in limbs.iter_mut().enumerate() {
        *limb = u64::from_le_bytes(input[i * 8..i * 8 + 8].try_into().unwrap());
    }
    limbs
}

/// `true` if `a >= b` (little-endian u64 limbs).
fn limbs_ge(a: &[u64; 4], b: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true
}

/// `a - b` (little-endian u64 limbs; requires `a >= b`).
fn limbs_sub(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    let mut out = [0u64; 4];
    let mut borrow = false;
    for i in 0..4 {
        let (d1, b1) = a[i].overflowing_sub(b[i]);
        let (d2, b2) = d1.overflowing_sub(borrow as u64);
        out[i] = d2;
        borrow = b1 || b2;
    }
    out
}

/// Reduce a 32-byte little-endian integer modulo the BN254 base field prime.
///
/// The prime is ~254 bits, so an arbitrary 256-bit input needs at most a few
/// subtractions. This keeps account IDs (256-bit Poseidon hashes) usable as
/// field elements deterministically, identical on host and guest.
pub fn reduce_mod_prime(input: &[u8; 32]) -> [u8; 32] {
    let mut limbs = to_u64_limbs(input);
    while limbs_ge(&limbs, &BN254_PRIME_LIMBS) {
        limbs = limbs_sub(&limbs, &BN254_PRIME_LIMBS);
    }
    let mut out = [0u8; 32];
    for (i, limb) in limbs.iter().enumerate() {
        out[i * 8..i * 8 + 8].copy_from_slice(&limb.to_le_bytes());
    }
    out
}

/// Hash a single 32-byte value as a BN254 field element.
///
/// The input is reduced modulo the base field prime first (account IDs and
/// other 256-bit values may exceed the ~254-bit prime), so any 32-byte input
/// is accepted.
pub fn hash_single(input: &[u8; 32]) -> [u8; 32] {
    let reduced = reduce_mod_prime(input);
    let felt = Felt::unsafe_from_le_bytes(&reduced);
    let out = compress_1(felt);
    Felt::to_le_bytes(&out)
}

/// Hash two 32-byte field elements (left, right).
///
/// # Panics
///
/// Panics if either input is not a valid BN254 field element.
pub fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let l = Felt::unsafe_from_le_bytes(left);
    let r = Felt::unsafe_from_le_bytes(right);
    assert!(
        Felt::is_valid(&l),
        "left input is not a valid BN254 field element"
    );
    assert!(
        Felt::is_valid(&r),
        "right input is not a valid BN254 field element"
    );
    let out = compress_2([l, r]);
    Felt::to_le_bytes(&out)
}

/// Compute default/empty hashes for a tree of `depth` levels.
///
/// Returns `defaults` indexed by level (0 = root, depth = leaves).
/// `defaults[depth] = ZERO`; `defaults[level] = H(defaults[level+1], defaults[level+1])`.
pub fn compute_default_hashes(depth: usize) -> Vec<[u8; 32]> {
    let mut defaults = vec![ZERO; depth + 1];
    defaults[depth] = ZERO;
    for level in (0..depth).rev() {
        let child = defaults[level + 1];
        defaults[level] = hash_pair(&child, &child);
    }
    defaults
}

/// A minimal sparse Merkle tree keyed by leaf index.
///
/// Only *leaf* nodes are stored; every internal node is derived on demand from
/// the current leaf set (with memoization per computation). This avoids the
/// classic sparse-tree bug where a previously-computed ancestor goes stale once
/// a later insert fills in a sibling that used to be a default subtree.
#[derive(Clone, Debug)]
pub struct SparseMerkleTree {
    depth: usize,
    /// Leaf-offset -> leaf hash. Leaf `i` lives at offset `(1 << depth) + i`.
    nodes: BTreeMap<usize, [u8; 32]>,
    /// Default hashes indexed by level (0 = root, depth = leaves).
    defaults: Vec<[u8; 32]>,
}

impl SparseMerkleTree {
    pub fn new(depth: usize) -> Self {
        Self {
            depth,
            nodes: BTreeMap::new(),
            defaults: compute_default_hashes(depth),
        }
    }

    pub fn with_default_depth() -> Self {
        Self::new(DEFAULT_TREE_DEPTH)
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    fn leaf_offset(&self, index: u64) -> usize {
        ((1usize << self.depth) as u64 + index) as usize
    }

    /// Leaf-offset range (half-open) for the subtree rooted at `(level, index)`.
    fn subtree_range(&self, level: usize, index: u64) -> (usize, usize) {
        let width = 1usize << (self.depth - level);
        let base = 1usize << self.depth;
        let start = (base as u64 + index * width as u64) as usize;
        (start, start + width)
    }

    /// True if any stored leaf lives under `(level, index)`.
    fn subtree_has_leaf(&self, level: usize, index: u64) -> bool {
        let (lo, hi) = self.subtree_range(level, index);
        self.nodes.range(lo..hi).next().is_some()
    }

    /// Hash of the node at `(level, index)`, derived from the stored leaves.
    fn node_hash(
        &self,
        level: usize,
        index: u64,
        memo: &mut BTreeMap<(usize, u64), [u8; 32]>,
    ) -> [u8; 32] {
        if let Some(h) = memo.get(&(level, index)) {
            return *h;
        }
        let h = if level == self.depth {
            self.nodes
                .get(&self.leaf_offset(index))
                .copied()
                .unwrap_or(self.defaults[self.depth])
        } else if !self.subtree_has_leaf(level, index) {
            self.defaults[level]
        } else {
            let left = self.node_hash(level + 1, index * 2, memo);
            let right = self.node_hash(level + 1, index * 2 + 1, memo);
            hash_pair(&left, &right)
        };
        memo.insert((level, index), h);
        h
    }

    /// Insert a leaf at `index`.
    pub fn insert(&mut self, index: u64, leaf: [u8; 32]) {
        self.nodes.insert(self.leaf_offset(index), leaf);
    }

    /// Current root hash.
    pub fn root(&self) -> [u8; 32] {
        let mut memo = BTreeMap::new();
        self.node_hash(0, 0, &mut memo)
    }

    /// Return the inclusion path (siblings, rootward) and leaf index for a leaf
    /// whose value matches `leaf`, or `None` if not present.
    pub fn find_path(&self, leaf: &[u8; 32]) -> Option<(Vec<[u8; 32]>, u64)> {
        for (offset, l) in &self.nodes {
            if l == leaf {
                let index = (*offset - (1usize << self.depth)) as u64;
                return Some((self.path(index), index));
            }
        }
        None
    }

    /// Build the inclusion path (siblings, rootward) for `index`.
    pub fn path(&self, index: u64) -> Vec<[u8; 32]> {
        let mut memo = BTreeMap::new();
        let mut path = Vec::with_capacity(self.depth);
        for level in (0..self.depth).rev() {
            let child_level = level + 1;
            let child_index = index >> (self.depth - child_level);
            path.push(self.node_hash(child_level, child_index ^ 1, &mut memo));
        }
        path
    }

    /// Verify an inclusion proof: `(path, index)` for `leaf` against `root`.
    ///
    /// Returns `true` iff the computed root matches `root`.
    pub fn verify_inclusion(
        root: &[u8; 32],
        leaf: &[u8; 32],
        path: &[[u8; 32]],
        index: u64,
    ) -> bool {
        let mut hash = *leaf;
        let mut idx = index;
        for sibling in path {
            hash = if idx & 1 == 0 {
                hash_pair(&hash, sibling)
            } else {
                hash_pair(sibling, &hash)
            };
            idx >>= 1;
        }
        hash == *root
    }

    /// Count of distinct leaves inserted.
    pub fn num_leaves(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_leaf(i: u8) -> [u8; 32] {
        let mut b = [0u8; 32];
        b[0] = i;
        b
    }

    #[test]
    fn empty_tree_root_is_full_depth_default_hash() {
        let tree = SparseMerkleTree::new(8);
        assert_eq!(tree.root(), compute_default_hashes(8)[0]);
        assert_ne!(tree.root(), ZERO);
    }

    #[test]
    fn default_leaf_verifies_against_default_root() {
        let tree = SparseMerkleTree::new(8);
        let path = tree.path(0);
        assert_eq!(path[0], ZERO);
        assert!(SparseMerkleTree::verify_inclusion(
            &tree.root(),
            &ZERO,
            &path,
            0
        ));
    }

    #[test]
    fn insert_moves_root() {
        let mut tree = SparseMerkleTree::new(8);
        let before = tree.root();
        tree.insert(0, test_leaf(1));
        assert_ne!(tree.root(), before);
    }

    #[test]
    fn path_verifies() {
        let mut tree = SparseMerkleTree::new(8);
        let leaves = [test_leaf(1), test_leaf(2), test_leaf(3), test_leaf(4)];
        for (i, l) in leaves.iter().enumerate() {
            tree.insert(i as u64, *l);
        }
        let root = tree.root();
        for (i, l) in leaves.iter().enumerate() {
            let path = tree.path(i as u64);
            assert!(SparseMerkleTree::verify_inclusion(
                &root, l, &path, i as u64
            ));
        }
    }

    #[test]
    fn wrong_leaf_fails_verification() {
        let mut tree = SparseMerkleTree::new(8);
        tree.insert(0, test_leaf(1));
        let root = tree.root();
        let path = tree.path(0);
        assert!(!SparseMerkleTree::verify_inclusion(
            &root,
            &test_leaf(2),
            &path,
            0
        ));
    }

    #[test]
    fn reduce_mod_prime_is_identity_for_small_values() {
        assert_eq!(reduce_mod_prime(&[7u8; 32]), [7u8; 32]);
        assert_eq!(reduce_mod_prime(&ZERO), ZERO);
    }

    #[test]
    fn reduce_mod_prime_handles_values_above_prime() {
        // all-0xff (max 256-bit) must reduce below the prime without panicking.
        let reduced = reduce_mod_prime(&[0xff; 32]);
        assert_ne!(reduced, [0xff; 32]);
        let _ = hash_single(&[0xff; 32]);
        // prime reduces to 0 (canonical < prime).
        let mut p = [0u8; 32];
        p[0..8].copy_from_slice(&0x3c20_8c16_d87c_fd47u64.to_le_bytes());
        p[8..16].copy_from_slice(&0x9781_6a91_6871_ca8du64.to_le_bytes());
        p[16..24].copy_from_slice(&0xb850_45b6_8181_585du64.to_le_bytes());
        p[24..32].copy_from_slice(&0x3064_4e72_e131_a029u64.to_le_bytes());
        assert_eq!(reduce_mod_prime(&p), ZERO);
        // reduction matches the independent subtract-prime loop.
        let mut limbs = to_u64_limbs(&[0xff; 32]);
        while limbs_ge(&limbs, &BN254_PRIME_LIMBS) {
            limbs = limbs_sub(&limbs, &BN254_PRIME_LIMBS);
        }
        let mut expected = [0u8; 32];
        for (i, limb) in limbs.iter().enumerate() {
            expected[i * 8..i * 8 + 8].copy_from_slice(&limb.to_le_bytes());
        }
        assert_eq!(reduced, expected);
    }
}
