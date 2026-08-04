//! Host client for the private airdrop on LEZ.
//!
//! Bins:
//! - [`airdrop_enroll`]: a recipient creates their hidden allocation account.
//! - [`airdrop_deploy`]: the distributor deploys the program + token and
//!   commits the eligibility root on-chain.
//! - [`airdrop_fund`]: the distributor mints each allocation into the hidden
//!   accounts (the on-chain "claim" the recipient then takes privately).
//! - [`airdrop_claim`]: a recipient moves their allocation into their own
//!   shielded account (nullifying the allocation commitment).
//! - [`airdrop_status`]: read/verify the on-chain distribution state.

pub mod airdrop;
