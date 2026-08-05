//! Recipient enrollment: creates the hidden allocation account `D_i` and the
//! recipient's personal shielded account, then writes an enrollment file for
//! the distributor.
//!
//! ```bash
//! cargo run --bin airdrop_enroll -- alice 1000000
//! ```

use airdrop::airdrop::types::Enrollment;
use airdrop::airdrop::{ENROLL_DIR, hex_account, hex32, init_wallet, private_account_identifier};

#[tokio::main]
async fn main() {
    let name = std::env::args()
        .nth(1)
        .expect("usage: airdrop_enroll <name> [amount]");
    let amount: u128 = std::env::args()
        .nth(2)
        .map(|a| a.parse().expect("amount must be a u128"))
        .unwrap_or(1_000_000);

    let mut wallet_core = init_wallet();

    // `D_i`: the hidden allocation account only this recipient can spend.
    let (d_account_id, _) = wallet_core.create_new_account_private(None);
    // The recipient's personal shielded account, the claim destination.
    let (main_account_id, _) = wallet_core.create_new_account_private(None);
    wallet_core
        .store_persistent_data()
        .expect("Failed to store wallet");

    let acc = wallet_core
        .storage()
        .key_chain()
        .private_account(d_account_id)
        .expect("D_i not found in key chain");
    let npk = acc.key_chain.nullifier_public_key;
    let vpk = acc.key_chain.viewing_public_key.clone();
    let identifier = private_account_identifier(&wallet_core, &d_account_id);

    let leaf = airdrop::airdrop::client::eligibility_leaf(&d_account_id);

    let enrollment = Enrollment {
        name: name.clone(),
        amount,
        d_account_id: hex_account(&d_account_id),
        main_account_id: hex_account(&main_account_id),
        npk: hex32(&npk.0),
        vpk: hex::encode(vpk.to_bytes()),
        identifier,
        leaf: hex32(&leaf),
    };

    std::fs::create_dir_all(ENROLL_DIR).expect("create enrollments dir");
    let path = format!("{ENROLL_DIR}/{name}.json");
    enrollment.to_file(&path);

    println!("Enrollment written to {path}");
    println!("  D_i:     {}", enrollment.d_account_id);
    println!("  main:    {}", enrollment.main_account_id);
    println!("  amount:  {}", enrollment.amount);
    println!("  leaf:    {}", enrollment.leaf);
    println!("Keep this wallet safe; it holds the secret keys for D_i.");
}
