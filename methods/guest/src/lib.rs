//! Guest crate for the private airdrop zkVM program.
//!
//! Pure handler logic lives in [`handlers`]; [`program`] holds the SPEL macro
//! wiring plus the `#[account_type]` struct definitions the framework scans.

// The spel `#[lez_program]` macro expands to a `.clone()` that newer (nightly)
// clippy flags as `cloned_ref_to_slice_refs`. It is generated code, not guest
// source, so allow it crate-wide.
#![allow(
    clippy::cloned_ref_to_slice_refs,
    reason = "spel #[lez_program] macro expansion, not editable in guest source"
)]

pub mod handlers;
pub mod program;
