//! Status/verify: prints the on-chain distribution state and re-verifies the
//! eligibility merkle root against the enrollment files.
//!
//! ```bash
//! cargo run --bin airdrop_status
//! ```

use airdrop::airdrop::client::read_distribution;
use airdrop::airdrop::types::{Enrollment, RunManifest};
use airdrop::airdrop::{ENROLL_DIR, MANIFEST_PATH, hex32, init_wallet, load_program};

#[tokio::main]
async fn main() {
    let wallet_core = init_wallet();
    let program = load_program();
    let manifest = RunManifest::from_file(MANIFEST_PATH);

    let state = read_distribution(&wallet_core, &program, manifest.distribution_id).await;

    println!("=== Distribution {} ===", manifest.distribution_id);
    println!("  program:            {}", manifest.airdrop_program_id);
    println!("  distribution acc:   {}", manifest.distribution_account);
    println!("  token definition:   {}", manifest.token_definition);
    println!("  distributor:        {}", manifest.distributor);
    println!("  committed at:       {}", state.committed_at);
    println!("  active:             {}", state.active);
    println!("  total allocation:   {}", state.total_allocation);
    println!("  num eligible:       {}", state.num_eligible);
    println!("  committed root:     {}", hex32(&state.root));
    println!("  manifest root:      {}", manifest.root);
    assert_eq!(
        hex32(&state.root),
        manifest.root,
        "on-chain root drifted from manifest"
    );

    // Rebuild the eligibility tree from enrollments and compare.
    let mut tree = airdrop_core::merkle::SparseMerkleTree::with_default_depth();
    let mut dir_entries: Vec<_> = std::fs::read_dir(ENROLL_DIR)
        .unwrap_or_else(|e| panic!("Failed to read {ENROLL_DIR}: {e}"))
        .filter_map(Result::ok)
        .collect();
    dir_entries.sort_by_key(|e| e.file_name());
    let mut count = 0u64;
    for entry in dir_entries {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let enroll = Enrollment::from_file(path.to_str().unwrap());
            let leaf = Enrollment::decode_hex32(&enroll.leaf, "leaf");
            tree.insert(count, leaf);
            count += 1;
        }
    }
    let rebuilt_root = tree.root();
    println!("  rebuilt root:       {}", hex32(&rebuilt_root));
    assert_eq!(
        hex32(&rebuilt_root),
        hex32(&state.root),
        "enrollment files no longer match the committed root"
    );
    println!("  eligibility root verified against {} enrollments.", count);
    println!("=== OK ===");
}
