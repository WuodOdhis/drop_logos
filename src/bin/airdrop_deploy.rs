//! Distributor deploy: reads enrollments, commits the eligibility root, deploys
//! the airdrop + token programs, creates the (hidden) token definition and
//! supply, and writes the run manifest.
//!
//! ```bash
//! cargo run --bin airdrop_deploy
//! ```

use airdrop::airdrop::{
    AIRDROP_BINARY, DATA_DIR, ENROLL_DIR, MANIFEST_PATH, hex32, hex_account, hex_program_id,
    init_wallet, load_program,
};
use airdrop::airdrop::client::{deploy_builtin_program, ensure_program_deployed, wait_for_block_seal};
use airdrop::airdrop::types::{Enrollment, RunManifest};
use wallet::program_facades::token::Token;

#[tokio::main]
async fn main() {
    let mut wallet_core = init_wallet();
    let program = load_program();
    let distribution_id: u64 = std::env::args()
        .nth(1)
        .map(|a| a.parse().expect("distribution_id must be a u64"))
        .unwrap_or(1);

    // ---- 1. Load enrollments and build the hidden eligibility set. ----
    let mut enrollments = Vec::new();
    let mut dir_entries: Vec<_> = std::fs::read_dir(ENROLL_DIR)
        .unwrap_or_else(|e| panic!("Failed to read {ENROLL_DIR}: {e}"))
        .filter_map(Result::ok)
        .collect();
    dir_entries.sort_by_key(|e| e.file_name());
    for entry in dir_entries {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            enrollments.push(Enrollment::from_file(path.to_str().unwrap()));
        }
    }
    assert!(!enrollments.is_empty(), "No enrollments found in {ENROLL_DIR}");

    let num_eligible = enrollments.len() as u64;
    let total_allocation: u128 = enrollments.iter().map(|e| e.amount).sum();
    println!(
        "Loaded {num_eligible} enrollments, total allocation {total_allocation}"
    );

    let leaves: Vec<[u8; 32]> = enrollments
        .iter()
        .map(|e| Enrollment::decode_hex32(&e.leaf, "leaf"))
        .collect();
    let mut tree = airdrop_core::merkle::SparseMerkleTree::with_default_depth();
    for (i, leaf) in leaves.iter().enumerate() {
        tree.insert(i as u64, *leaf);
    }
    let root = tree.root();
    for (i, leaf) in leaves.iter().enumerate() {
        let path = tree.path(i as u64);
        assert!(
            airdrop_core::merkle::SparseMerkleTree::verify_inclusion(&root, leaf, &path, i as u64),
            "enrollment {i} leaf does not verify against root"
        );
    }
    println!("Eligibility root: {}", hex32(&root));

    // ---- 2. Distributor public account (signs the distribution tx). ----
    let (distributor, _) = wallet_core.create_new_account_public(None);
    wallet_core
        .store_persistent_data()
        .expect("Failed to store wallet");

    // ---- 3. Deploy programs (idempotent). ----
    let distribution_account = airdrop::airdrop::client::distribution_account(&program, distribution_id);
    ensure_program_deployed(
        &wallet_core,
        &program,
        AIRDROP_BINARY,
        "Airdrop program",
        &distribution_account,
    )
    .await;
    wait_for_block_seal().await;

    deploy_builtin_program(&wallet_core, &programs::token(), "Token program").await;
    wait_for_block_seal().await;

    deploy_builtin_program(
        &wallet_core,
        &programs::authenticated_transfer(),
        "Authenticated transfer program",
    )
    .await;
    wait_for_block_seal().await;

    // ---- 4. Hidden token definition + supply (private accounts). ----
    let (definition_id, _) = wallet_core.create_new_account_private(None);
    let (supply_id, _) = wallet_core.create_new_account_private(None);
    wallet_core
        .store_persistent_data()
        .expect("Failed to store wallet");

    Token(&wallet_core)
        .send_new_definition_private_owned_definiton_and_supply(
            definition_id.clone(),
            supply_id.clone(),
            "Private Airdrop".to_string(),
            total_allocation,
        )
        .await
        .expect("Failed to create token definition");
    wallet_core
        .sync_to_latest_block()
        .await
        .expect("Failed to sync after token definition");

    // ---- 5. Commit the root on-chain. ----
    let distribution_account = airdrop::airdrop::client::initialize_distribution(
        &wallet_core,
        &program,
        distribution_id,
        &distributor,
        root,
        *definition_id.value(),
        total_allocation,
        num_eligible,
    )
    .await;

    // ---- 6. Write the run manifest. ----
    let manifest = RunManifest {
        distribution_id,
        airdrop_program_id: hex_program_id(&program.id()),
        distribution_account: hex_account(&distribution_account),
        token_definition: hex_account(&definition_id),
        supply_holding: hex_account(&supply_id),
        distributor: hex_account(&distributor),
        root: hex32(&root),
        total_allocation,
        num_eligible,
    };
    std::fs::create_dir_all(DATA_DIR).expect("create data dir");
    manifest.to_file(MANIFEST_PATH);
    println!("Manifest written to {MANIFEST_PATH}");
    println!("Next: run `airdrop_fund` to mint each allocation into its hidden account.");
}
