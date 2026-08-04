//! Distributor funding: mints each recipient's allocation into their hidden
//! account `D_i`. This is the distributor-side commit; the recipient then
//! "claims" by spending `D_i` into their own shielded account.
//!
//! ```bash
//! cargo run --bin airdrop_fund
//! ```

use airdrop::airdrop::{
    ENROLL_DIR, MANIFEST_PATH, init_wallet, load_program,
};
use airdrop::airdrop::types::{Enrollment, RunManifest};
use nssa::AccountId;
use nssa_core::encryption::ViewingPublicKey;
use nssa_core::NullifierPublicKey;
use wallet::program_facades::token::Token;

fn read_enrollments() -> Vec<Enrollment> {
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
    enrollments
}

#[tokio::main]
async fn main() {
    let mut wallet_core = init_wallet();
    let _program = load_program();
    let manifest = RunManifest::from_file(MANIFEST_PATH);
    let enrollments = read_enrollments();
    assert!(!enrollments.is_empty(), "No enrollments found in {ENROLL_DIR}");

    let definition_id = AccountId::new(airdrop::airdrop::client::parse_hex32(
        &manifest.token_definition,
        "token_definition",
    ));

    wallet_core
        .sync_to_latest_block()
        .await
        .expect("Failed to sync to latest block");

    // The token definition created in `airdrop_deploy` may not be decrypted
    // into the key chain yet (block-seal race); wait until it holds valid data.
    let definition_ok = |w: &wallet::WalletCore| -> bool {
        w.get_account_private(definition_id.clone())
            .and_then(|acc| token_core::TokenDefinition::try_from(&acc.data).ok())
            .is_some()
    };
    for _ in 0..airdrop::airdrop::client::wait_account_attempts() {
        if definition_ok(&wallet_core) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        wallet_core
            .sync_to_latest_block()
            .await
            .expect("Failed to sync while waiting for token definition");
    }
    assert!(
        definition_ok(&wallet_core),
        "Token definition not available in key chain after funding sync"
    );

    // Each mint spends the definition account's commitment and increments its
    // `total_supply`, so the next mint must be built from the updated
    // definition post-state. Without syncing (and waiting for the previous
    // mint's block to seal) all mints are built from the same pre-state and
    // the sequencer rejects the duplicate definition commitment as
    // "Commitment already seen".
    let total_supply = |w: &wallet::WalletCore| -> u128 {
        w.get_account_private(definition_id.clone())
            .and_then(|acc| token_core::TokenDefinition::try_from(&acc.data).ok())
            .and_then(|def| match def {
                token_core::TokenDefinition::Fungible { total_supply, .. } => Some(total_supply),
                _ => None,
            })
            .unwrap_or(0)
    };
    let mut expected_supply = total_supply(&wallet_core);

    for enroll in &enrollments {
        let npk = NullifierPublicKey(Enrollment::decode_hex32(&enroll.npk, "npk"));
        let vpk_bytes = hex::decode(&enroll.vpk)
            .unwrap_or_else(|e| panic!("vpk for {} is not valid hex: {e}", enroll.name));
        let vpk = ViewingPublicKey::from_bytes(vpk_bytes)
            .expect("vpk must be a valid ML-KEM-768 encapsulation key");

        println!(
            "Funding {} ({} tokens) into D_i {}",
            enroll.name, enroll.amount, enroll.d_account_id
        );
        Token(&wallet_core)
            .send_mint_transaction_private_foreign_account(
                definition_id.clone(),
                npk,
                vpk,
                enroll.identifier,
                enroll.amount,
            )
            .await
            .expect("Failed to mint allocation into D_i");

        // Wait until the minted block is sealed and synced so the definition
        // post-state reflects this mint before building the next one.
        expected_supply = expected_supply.saturating_add(enroll.amount);
        for _ in 0..airdrop::airdrop::client::wait_account_attempts() {
            wallet_core
                .sync_to_latest_block()
                .await
                .expect("Failed to sync between mint transactions");
            if total_supply(&wallet_core) == expected_supply {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        assert_eq!(
            total_supply(&wallet_core),
            expected_supply,
            "Mint for {} was not sealed; definition total_supply is stale",
            enroll.name
        );
    }

    wallet_core
        .sync_to_latest_block()
        .await
        .expect("Failed to sync after funding");

    println!("All {} allocations minted.", enrollments.len());
    println!("Recipients: run `airdrop_claim --name <name>` (from the recipient's wallet dir).");
}
