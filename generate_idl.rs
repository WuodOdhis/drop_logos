//! Print the airdrop program IDL as JSON.
//!
//! The `spel_framework::generate_idl!` macro parses `program.rs`, finds the
//! `#[lez_program]` module, and emits a `fn main()` that prints the full IDL
//! (instructions, account layouts, PDA seeds) as JSON.
//!
//! ```bash
//! cargo run --bin generate_idl --manifest-path Cargo.toml \
//!   > idl/airdrop_program_idl.json
//! ```

spel_framework::generate_idl!("methods/guest/src/program.rs");
