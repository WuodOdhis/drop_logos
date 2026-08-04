//! Client helpers for the private airdrop (shared by all bins).

use std::time::Duration;

use common::transaction::LeeTransaction as NSSATransaction;
use nssa::{
    AccountId, ProgramDeploymentTransaction, PublicTransaction,
    program::Program,
    program_deployment_transaction,
    public_transaction::{Message, WitnessSet},
};
use nssa_core::{
    program::ProgramId,
    PrivateAccountKind,
};
use sequencer_service_rpc::RpcClient as _;
use wallet::WalletCore;

use crate::airdrop::pda::distribution_account_id;

/// Deploy artifact produced by the local (non-docker) risc0 guest build in
/// `methods/` (via `cargo build` in `airdrop/methods`). The docker-path
/// artifact (`methods/guest/target/.../docker/airdrop.bin`) is only produced by
/// `cargo risczero build`, which needs Docker BuildKit (buildx); the local build
/// output below is used instead.
pub const AIRDROP_BINARY: &str =
    "methods/target/riscv-guest/airdrop_methods/airdrop_guest/riscv32im-risc0-zkvm-elf/release/airdrop.bin";
/// Local state dir (relative to `airdrop/` when running the bins).
pub const DATA_DIR: &str = ".logos-airdrop";
/// Where `airdrop_enroll` writes recipient files.
pub const ENROLL_DIR: &str = ".logos-airdrop/enrollments";
/// Where `airdrop_deploy` writes the run descriptor.
pub const MANIFEST_PATH: &str = ".logos-airdrop/run.json";

/// CLOCK_50 system account id.
pub fn clock_account_id() -> AccountId {
    AccountId::new(airdrop_core::CLOCK_50_ACCOUNT_ID_BYTES)
}

/// Initialize a WalletCore, creating storage if missing.
///
/// An existing `wallet_config.json` is preserved; with none present, a fresh
/// local-dev default is written.
pub fn init_wallet() -> WalletCore {
    let config_path = wallet::helperfunctions::fetch_config_path().unwrap();
    let storage_path = wallet::helperfunctions::fetch_persistent_storage_path().unwrap();
    if storage_path.exists() {
        WalletCore::new_update_chain(config_path, storage_path, None).unwrap()
    } else {
        println!("First run: initializing wallet storage at {storage_path:?}");
        WalletCore::new_init_storage(config_path, storage_path, None, "")
            .unwrap()
            .0
    }
}

/// Load the airdrop program from the deploy artifact.
pub fn load_program() -> Program {
    let bytecode = std::fs::read(AIRDROP_BINARY).unwrap_or_else(|e| {
        panic!(
            "Failed to read airdrop program binary from {AIRDROP_BINARY}: {e}\n\
             Build it first: cargo risczero build (in methods/) then cargo build."
        )
    });
    Program::new(bytecode.into()).expect("Failed to parse airdrop program")
}

/// Sleep long enough for the sequencer to seal a block between deployments.
/// Override via `LEZ_AIRDROP_BLOCK_SEAL_SECS`.
pub async fn wait_for_block_seal() {
    let secs = std::env::var("LEZ_AIRDROP_BLOCK_SEAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    tokio::time::sleep(Duration::from_secs(secs)).await;
}

/// Default `max_attempts` for `wait_for_account_data`. Each attempt sleeps
/// 500 ms. Override via `LEZ_AIRDROP_ACCOUNT_WAIT_ATTEMPTS`.
pub fn wait_account_attempts() -> u32 {
    std::env::var("LEZ_AIRDROP_ACCOUNT_WAIT_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(240)
}

/// Wait for a public account to have non-empty data.
pub async fn wait_for_account_data(
    wallet_core: &WalletCore,
    account_id: &AccountId,
    max_attempts: u32,
) {
    for _ in 0..max_attempts {
        let account = wallet_core
            .get_account_public(account_id.clone())
            .await
            .expect("Failed to fetch account");
        if !account.data.as_ref().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!(
        "Timeout waiting for account {} to be initialized",
        account_id
    );
}

/// Distribution account id for `distribution_id` under `program`.
pub fn distribution_account(program: &Program, distribution_id: u64) -> AccountId {
    distribution_account_id(&program.id(), distribution_id)
}

/// True if `program`'s `distribution_id` PDA already has on-chain data.
pub async fn is_initialized(wallet_core: &WalletCore, program: &Program, distribution_id: u64) -> bool {
    let id = distribution_account(program, distribution_id);
    let account = wallet_core
        .get_account_public(id)
        .await
        .expect("Failed to fetch distribution account from sequencer");
    !account.data.as_ref().is_empty()
}

async fn is_program_deployed(
    wallet_core: &WalletCore,
    program: &Program,
    account_id: &AccountId,
) -> bool {
    match wallet_core.get_account_public(account_id.clone()).await {
        Ok(account) => account.program_owner == program.id(),
        Err(_) => false,
    }
}

async fn send_deploy_tx(wallet_core: &WalletCore, program: &Program, program_name: &str, bytecode: Vec<u8>) {
    let deploy_msg = program_deployment_transaction::Message::new(bytecode);
    let deploy_tx = ProgramDeploymentTransaction::new(deploy_msg);

    match wallet_core
        .sequencer_client
        .send_transaction(NSSATransaction::ProgramDeployment(deploy_tx))
        .await
    {
        Ok(_) => println!(
            "  {} deployed (program ID: {:?})",
            program_name,
            program.id()
        ),
        Err(e) => {
            let err_str = format!("{:?}", e);
            if err_str.contains("already")
                || err_str.contains("exists")
                || err_str.contains("duplicate")
            {
                println!(
                    "  {} already deployed (program ID: {:?})",
                    program_name,
                    program.id()
                );
            } else {
                panic!("Failed to deploy {}: {:?}", program_name, e);
            }
        }
    }
}

/// Deploy `program` if it isn't already deployed (checked via `check_account`).
pub async fn ensure_program_deployed(
    wallet_core: &WalletCore,
    program: &Program,
    bytecode_path: &str,
    program_name: &str,
    check_account: &AccountId,
) {
    if is_program_deployed(wallet_core, program, check_account).await {
        println!(
            "  {} already deployed (program ID: {:?})",
            program_name,
            program.id()
        );
        return;
    }

    let bytecode = std::fs::read(bytecode_path).unwrap_or_else(|_| {
        panic!(
            "Failed to read {} binary from {}",
            program_name, bytecode_path
        )
    });

    let loaded_program = Program::new(bytecode.clone().into())
        .unwrap_or_else(|_| panic!("Failed to parse {} binary", program_name));

    if loaded_program.id() != program.id() {
        panic!(
            "{} bytecode mismatch: expected program ID {:?}, got {:?}. \
             The binary at {} doesn't match the expected program.",
            program_name,
            program.id(),
            loaded_program.id(),
            bytecode_path
        );
    }

    send_deploy_tx(wallet_core, program, program_name, bytecode).await;
}

/// Deploy a builtin program (token / authenticated_transfer) idempotently.
pub async fn deploy_builtin_program(wallet_core: &WalletCore, program: &Program, program_name: &str) {
    send_deploy_tx(wallet_core, program, program_name, program.elf().to_vec()).await;
}

/// `initialize_distribution` public transaction: commits the eligibility root.
/// `distributor` must be a wallet-owned public account (it signs).
pub async fn initialize_distribution(
    wallet_core: &WalletCore,
    program: &Program,
    distribution_id: u64,
    distributor: &AccountId,
    root: [u8; 32],
    token_definition: [u8; 32],
    total_allocation: u128,
    num_eligible: u64,
) -> AccountId {
    let distribution = distribution_account(program, distribution_id);
    let accounts = vec![
        distribution.clone(),
        distributor.clone(),
        clock_account_id(),
    ];

    let signing_key = wallet_core
        .get_account_public_signing_key(distributor.clone())
        .expect("Distributor account not found in wallet");
    let nonces = wallet_core
        .get_accounts_nonces(vec![distributor.clone()])
        .await
        .expect("Failed to fetch distributor nonce");

    let instruction = airdrop_core::Instruction::InitializeDistribution {
        distribution_id,
        root,
        token_definition,
        total_allocation,
        num_eligible,
    };

    let message = Message::try_new(program.id(), accounts, nonces, instruction)
        .expect("Failed to create initialize_distribution message");
    let witness_set = WitnessSet::for_message(&message, &[signing_key]);
    let tx = PublicTransaction::new(message, witness_set);

    let hash = wallet_core
        .sequencer_client
        .send_transaction(NSSATransaction::Public(tx))
        .await
        .expect("Failed to send initialize_distribution");
    println!("  initialize_distribution tx hash: {hash}");

    wait_for_account_data(wallet_core, &distribution, wait_account_attempts()).await;
    println!("  distribution account {} initialized", distribution);
    distribution
}

/// `freeze_distribution` public transaction. `distributor` signs.
pub async fn freeze_distribution(
    wallet_core: &WalletCore,
    program: &Program,
    distribution_id: u64,
    distributor: &AccountId,
) {
    let distribution = distribution_account(program, distribution_id);
    let accounts = vec![distribution.clone(), distributor.clone(), clock_account_id()];

    let signing_key = wallet_core
        .get_account_public_signing_key(distributor.clone())
        .expect("Distributor account not found in wallet");
    let nonces = wallet_core
        .get_accounts_nonces(vec![distributor.clone()])
        .await
        .expect("Failed to fetch distributor nonce");

    let instruction = airdrop_core::Instruction::FreezeDistribution { distribution_id };

    let message = Message::try_new(program.id(), accounts, nonces, instruction)
        .expect("Failed to create freeze_distribution message");
    let witness_set = WitnessSet::for_message(&message, &[signing_key]);
    let tx = PublicTransaction::new(message, witness_set);

    let hash = wallet_core
        .sequencer_client
        .send_transaction(NSSATransaction::Public(tx))
        .await
        .expect("Failed to send freeze_distribution");
    println!("  freeze_distribution tx hash: {hash}");
}

/// Read and decode the on-chain `DistributionState`.
pub async fn read_distribution(
    wallet_core: &WalletCore,
    program: &Program,
    distribution_id: u64,
) -> airdrop_core::DistributionState {
    let id = distribution_account(program, distribution_id);
    let account = wallet_core
        .get_account_public(id)
        .await
        .expect("Failed to fetch distribution account");
    let data = account.data.as_ref();
    assert!(!data.is_empty(), "Distribution {} is not initialized", id);
    borsh::from_slice(data).expect("Decode DistributionState")
}

/// Hex of an account id.
pub fn hex_account(id: &AccountId) -> String {
    id.value().iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex of a 32-byte value.
pub fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse a hex string into `[u8; 32]`.
pub fn parse_hex32(s: &str, what: &str) -> [u8; 32] {
    let bytes = hex::decode(s).unwrap_or_else(|e| panic!("{what} is not valid hex: {e}"));
    bytes.try_into().unwrap_or_else(|v: Vec<u8>| {
        panic!("{what} must decode to 32 bytes, got {}", v.len())
    })
}

/// Extract the numeric identifier for a private account in the wallet key chain.
///
/// `D_i` is a regular private account (identifier `0` for a fresh key node),
/// but we read it from the stored `PrivateAccountKind` to stay honest against
/// any future identifier policy.
pub fn private_account_identifier(wallet_core: &WalletCore, account_id: &AccountId) -> u128 {
    let acc = wallet_core
        .storage()
        .key_chain()
        .private_account(account_id.clone())
        .expect("Private account not found in wallet");
    match acc.kind {
        PrivateAccountKind::Regular(identifier) => *identifier,
        PrivateAccountKind::Pda { .. } => {
            panic!("airdrop allocations must be regular private accounts, got a PDA")
        }
    }
}

/// Derive the eligibility merkle leaf for an account id: `H(account_id)`.
/// Poseidon hashing matches the guest via `airdrop_core::merkle`.
pub fn eligibility_leaf(account_id: &AccountId) -> [u8; 32] {
    airdrop_core::merkle::hash_single(account_id.value())
}

/// ProgramId hex (for manifests / status).
pub fn hex_program_id(id: &ProgramId) -> String {
    let bytes: [u8; 32] = bytemuck::cast(*id);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Build the eligibility SMT from enrollment leaves and return the root.
pub fn eligibility_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    let mut tree = airdrop_core::merkle::SparseMerkleTree::with_default_depth();
    for (i, leaf) in leaves.iter().enumerate() {
        tree.insert(i as u64, *leaf);
    }
    tree.root()
}
