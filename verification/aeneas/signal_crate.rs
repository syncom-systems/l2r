//! Extraction harness: a crate consisting of exactly the committed `signal`
//! module of the crossing example (contract `mod.rs` plus the implementer's
//! `imp.rs`), so Charon and Aeneas translate the code that is built and tested,
//! not a copy of it.

#[path = "../../examples/crossing/scaffold/src/crossing/signal/mod.rs"]
pub mod signal;
