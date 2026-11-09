//! Extraction harness: a crate made of exactly the committed implementation
//! modules of the crossing example: both machines (generated contract `mod.rs`
//! plus the implementer's `imp.rs`) and the message translations (`routes.rs`).
//! Charon and Aeneas translate the code that is built and tested, not a copy.
//! The composite scheduler (`crossing/mod.rs`) is deliberately excluded; see
//! README.md for why.

#[path = "../../examples/crossing/scaffold/src/crossing/request_latch/mod.rs"]
pub mod request_latch;
#[path = "../../examples/crossing/scaffold/src/crossing/routes.rs"]
pub mod routes;
#[path = "../../examples/crossing/scaffold/src/crossing/signal/mod.rs"]
pub mod signal;
