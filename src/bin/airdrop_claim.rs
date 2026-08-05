//! Recipient claim: moves the allocation out of `D_i` into the recipient's own
//! shielded account. Spending `D_i` publishes its nullifier, so a second claim
//! is impossible (demonstrated at the end of the run).
//!
//! ```bash
//! cargo run --bin airdrop_claim -- --name alice
//! ```

use airdrop::airdrop::types::{Enrollment, RunManifest};
use airdrop::airdrop::{
    ENROLL_DIR, MANIFEST_PATH,
    client::{read_distribution, wait_account_attempts},
    init_wallet, load_program,
};
use nssa::AccountId;
use std::time::Duration;
use token_core::TokenHolding;
use wallet::program_facades::token::Token;

fn parse_args() -> String {
    let args: Vec<String> = std::env::args().collect();
    let mut name = None;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--name" {
            name = iter.next().cloned();
        }
    }
    name.expect("usage: airdrop_claim --name <name>")
}

/// Read the fungible token balance of a private account from the wallet's
/// decrypted key-chain state (after sync). Returns `None` if the account is
/// not in the key chain yet or does not hold token data yet.
fn private_token_balance(wallet_core: &wallet::WalletCore, account_id: &AccountId) -> Option<u128> {
    let account = wallet_core.get_account_private(*account_id)?;
    match TokenHolding::try_from(&account.data).ok()? {
        TokenHolding::Fungible { balance, .. } => Some(balance),
        TokenHolding::NftMaster { .. } | TokenHolding::NftPrintedCopy { .. } => None,
    }
}

#[tokio::main]
async fn main() {
    let name = parse_args();
    let mut wallet_core = init_wallet();
    let program = load_program();
    let manifest = RunManifest::from_file(MANIFEST_PATH);

    let enrollment = Enrollment::from_file(&format!("{ENROLL_DIR}/{name}.json"));

    // The distribution must be live (committed, not frozen).
    let state = read_distribution(&wallet_core, &program, manifest.distribution_id).await;
    assert!(state.is_active(), "Distribution is frozen; claims disabled");

    // Sync so the wallet decrypts the funding minted into D_i.
    println!("Syncing wallet...");
    wallet_core
        .sync_to_latest_block()
        .await
        .expect("Failed to sync to latest block");

    let d_account_id = AccountId::new(airdrop::airdrop::client::parse_hex32(
        &enrollment.d_account_id,
        "d_account_id",
    ));
    let main_account_id = AccountId::new(airdrop::airdrop::client::parse_hex32(
        &enrollment.main_account_id,
        "main_account_id",
    ));

    // The funding minted by the distributor may not be decrypted into the
    // key chain yet (block-seal race); poll until D_i holds the allocation.
    let mut d_balance = private_token_balance(&wallet_core, &d_account_id);
    for _ in 0..wait_account_attempts() {
        if d_balance == Some(enrollment.amount) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        wallet_core
            .sync_to_latest_block()
            .await
            .expect("Failed to sync while waiting for D_i funding");
        d_balance = private_token_balance(&wallet_core, &d_account_id);
    }
    let d_balance = d_balance.expect("D_i should hold a token balance after funding");
    println!(
        "D_i {} balance before claim: {d_balance}",
        enrollment.d_account_id
    );
    assert_eq!(
        d_balance, enrollment.amount,
        "D_i balance does not match the allocation; was it funded?"
    );

    // ---- Claim: D_i -> main shielded account. ----
    println!(
        "Claiming {} tokens from D_i into the recipient's shielded account...",
        enrollment.amount
    );
    let (hash, _secrets) = Token(&wallet_core)
        .send_transfer_transaction_private_owned_account(
            d_account_id,
            main_account_id,
            enrollment.amount,
        )
        .await
        .expect("Failed to claim allocation");
    println!("  claim tx hash: {hash}");

    wallet_core
        .sync_to_latest_block()
        .await
        .expect("Failed to sync after claim");

    // The claim lands in a block that may not be sealed yet; poll until the
    // recipient's shielded account holds the token data.
    let mut main_balance = private_token_balance(&wallet_core, &main_account_id);
    for _ in 0..wait_account_attempts() {
        if main_balance == Some(enrollment.amount) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        wallet_core
            .sync_to_latest_block()
            .await
            .expect("Failed to sync while waiting for claim");
        main_balance = private_token_balance(&wallet_core, &main_account_id);
    }
    println!(
        "  main shielded account balance after claim: {}",
        main_balance.unwrap_or(0)
    );
    assert_eq!(main_balance, Some(enrollment.amount), "Claim did not land");

    // ---- Double-claim must fail: D_i's nullifier is already spent. ----
    println!("Attempting a second claim (must fail)...");
    let double_claim = Token(&wallet_core)
        .send_transfer_transaction_private_owned_account(d_account_id, main_account_id, 1)
        .await;
    match double_claim {
        Ok(_) => panic!("Double claim unexpectedly succeeded"),
        Err(e) => {
            let err_str = format!("{e:?}");
            println!("  double claim rejected: {err_str}");
            assert!(
                err_str.to_lowercase().contains("nullifier")
                    || err_str.to_lowercase().contains("already")
                    || err_str.to_lowercase().contains("spent")
                    || err_str.to_lowercase().contains("insufficient"),
                "expected a nullifier/spent error, got: {err_str}"
            );
            println!("  OK: double claim prevented by the on-chain nullifier set.");
        }
    }
}
