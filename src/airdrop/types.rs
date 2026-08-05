//! Serde types for enrollment files exchanged between recipients and the
//! distributor, plus a small manifest for a distribution run.

use serde::{Deserialize, Serialize};

/// One recipient's hidden allocation account, as written by `airdrop_enroll`
/// and consumed by `airdrop_deploy`/`airdrop_fund`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Enrollment {
    /// Human-readable recipient label (demo only).
    pub name: String,
    /// Allocation amount (token units).
    pub amount: u128,
    /// Hex (64 chars) of the hidden allocation account id `D_i`.
    pub d_account_id: String,
    /// Hex (64 chars) of the recipient's personal shielded account id.
    pub main_account_id: String,
    /// Hex (64 chars) nullifier public key of `D_i`.
    pub npk: String,
    /// Hex (64 chars) viewing public key of `D_i`.
    pub vpk: String,
    /// Identifier of `D_i` (part of the account-id derivation).
    pub identifier: u128,
    /// Hex (64 chars) of the eligibility merkle leaf for this recipient.
    pub leaf: String,
}

impl Enrollment {
    pub fn from_file(path: &str) -> Self {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read enrollment {path}: {e}"));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("Invalid enrollment {path}: {e}"))
    }

    pub fn to_file(&self, path: &str) {
        let raw = serde_json::to_string_pretty(self).expect("serialize enrollment");
        std::fs::write(path, raw).expect("write enrollment");
    }

    pub fn decode_hex32(s: &str, what: &str) -> [u8; 32] {
        let bytes = hex::decode(s).unwrap_or_else(|e| panic!("{what} is not valid hex: {e}"));
        bytes
            .try_into()
            .unwrap_or_else(|v: Vec<u8>| panic!("{what} must decode to 32 bytes, got {}", v.len()))
    }
}

/// A distribution run descriptor, written by `airdrop_deploy`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RunManifest {
    pub distribution_id: u64,
    /// Hex of the airdrop program id.
    pub airdrop_program_id: String,
    /// Hex of the distribution account id.
    pub distribution_account: String,
    /// Hex of the token definition account id.
    pub token_definition: String,
    /// Hex of the token supply account id.
    pub supply_holding: String,
    /// Hex of the distributor account id.
    pub distributor: String,
    pub root: String,
    pub total_allocation: u128,
    pub num_eligible: u64,
}

impl RunManifest {
    pub fn from_file(path: &str) -> Self {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read manifest {path}: {e}"));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("Invalid manifest {path}: {e}"))
    }

    pub fn to_file(&self, path: &str) {
        let raw = serde_json::to_string_pretty(self).expect("serialize manifest");
        std::fs::write(path, raw).expect("write manifest");
    }
}
