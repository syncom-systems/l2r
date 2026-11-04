//! Extraction harness for the `composite/v1` proof lane: the committed, generated
//! composite of `examples/crossing_v1` (scheduler `System`, array-backed mailboxes,
//! both machines and the message translations), exactly as built and tested.
//! Charon and Aeneas translate the code that ships, not a copy.

#[path = "../../examples/crossing_v1/scaffold/src/crossing/mod.rs"]
pub mod crossing;
