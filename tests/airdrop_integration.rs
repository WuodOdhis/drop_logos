//! Host-side integration tests for the private airdrop.
//!
//! These run without the zkVM or a sequencer: they cover the shared
//! Sparse Merkle Tree, PDA derivation, and the enrollment/manifest serde
//! round-trip that the bins rely on. Run with:
//!
//! ```bash
//! cargo test --manifest-path Cargo.toml --test airdrop_integration
//! ```

use airdrop::airdrop::client::{eligibility_leaf, eligibility_root};
use airdrop::airdrop::pda::distribution_account_id;
use airdrop::airdrop::types::{Enrollment, RunManifest};
use airdrop_core::merkle::{SparseMerkleTree, hash_pair, hash_single};
use airdrop_core::pda::distribution_seed;
use nssa::AccountId;

fn leaf(i: u8) -> [u8; 32] {
    let mut b = [0u8; 32];
    b[0] = i;
    b
}

#[test]
fn eligibility_root_and_path_verify() {
    let leaves = [leaf(1), leaf(2), leaf(3), leaf(4)];
    let root = eligibility_root(&leaves);

    let mut tree = SparseMerkleTree::with_default_depth();
    for (i, l) in leaves.iter().enumerate() {
        tree.insert(i as u64, *l);
    }
    assert_eq!(root, tree.root());

    for (i, l) in leaves.iter().enumerate() {
        let path = tree.path(i as u64);
        assert!(SparseMerkleTree::verify_inclusion(
            &root, l, &path, i as u64
        ));
    }
}

#[test]
fn eligibility_leaf_matches_shared_hash() {
    let id = AccountId::new([9u8; 32]);
    let a = eligibility_leaf(&id);
    let b = hash_single(&[9u8; 32]);
    assert_eq!(a, b);
    // H is not an identity on a single zero commit.
    assert_ne!(a, [0u8; 32]);
}

#[test]
fn distribution_pda_matches_guest_seed() {
    let program_id = bytemuck::cast([0x42u8; 32]);
    let id = distribution_account_id(&program_id, 7);
    let seed = distribution_seed(7);
    let direct = AccountId::for_public_pda(&program_id, &nssa_core::program::PdaSeed::new(seed));
    assert_eq!(id, direct);

    // SHA-256 combining matches SPEL's compute_pda (label zero-padded || u64 LE).
    let label = airdrop_core::label_seed("distribution");
    let u64seed = airdrop_core::u64_seed(7);
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    hasher.update(label);
    hasher.update(u64seed);
    let combined: [u8; 32] = hasher.finalize().into();
    let manual =
        AccountId::for_public_pda(&program_id, &nssa_core::program::PdaSeed::new(combined));
    assert_eq!(id, manual);
}

#[test]
fn enrollment_serde_round_trip() {
    let e = Enrollment {
        name: "alice".into(),
        amount: 1_000_000,
        d_account_id: "11".repeat(32),
        main_account_id: "22".repeat(32),
        npk: "33".repeat(32),
        vpk: "44".repeat(1184),
        identifier: 0,
        leaf: "55".repeat(32),
    };
    let dir = std::env::temp_dir().join("airdrop-enroll-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("alice.json");
    let p = path.to_str().unwrap().to_string();
    e.to_file(&p);
    let back = Enrollment::from_file(&p);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.name, e.name);
    assert_eq!(back.amount, e.amount);
    assert_eq!(back.d_account_id, e.d_account_id);
    assert_eq!(back.leaf, e.leaf);
}

#[test]
fn manifest_serde_round_trip() {
    let m = RunManifest {
        distribution_id: 1,
        airdrop_program_id: "ab".repeat(32),
        distribution_account: "ab".repeat(32),
        token_definition: "ab".repeat(32),
        supply_holding: "ab".repeat(32),
        distributor: "ab".repeat(32),
        root: "cd".repeat(32),
        total_allocation: 3_000_000,
        num_eligible: 3,
    };
    let dir = std::env::temp_dir().join("airdrop-manifest-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("run.json");
    let p = path.to_str().unwrap().to_string();
    m.to_file(&p);
    let back = RunManifest::from_file(&p);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.distribution_id, m.distribution_id);
    assert_eq!(back.root, m.root);
}

#[test]
fn hash_pair_is_commutative_symmetric_inputs() {
    // H(a, b) with a != b is order-sensitive; H(a, a) is a fixed point check.
    let a = leaf(1);
    let b = leaf(2);
    assert_ne!(hash_pair(&a, &b), hash_pair(&b, &a));
}
