//! Shared no_std types for the private airdrop.
//!
//! This crate is consumed by both the LEZ guest program (`methods/guest`) and
//! the host client (`src`). It defines:
//!
//! - [`DistributionState`]: the on-chain distribution configuration.
//! - [`Instruction`]: the serde `Instruction` enum passed between host and guest.
//! - PDA seed helpers ([`label_seed`], [`u64_seed`], [`combine_seeds`]).
//! - A Poseidon Sparse Merkle Tree ([`SparseMerkleTree`]) over the hidden
//!   eligibility set.
//!
//! # no_std Support
//!
//! Disable the default `std` feature for guest/embedded targets:
//!
//! ```toml
//! airdrop-core = { path = "../airdrop-core", default-features = false }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

pub mod constants;
pub mod instruction;
pub mod merkle;
pub mod pda;
pub mod state;

pub use constants::*;
pub use instruction::Instruction;
pub use pda::{combine_seeds, distribution_pda_seed, distribution_seed, label_seed, u64_seed};
pub use state::DistributionState;
