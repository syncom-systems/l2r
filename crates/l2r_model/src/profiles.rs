//! Identities of the versioned generation profiles a model may select.
//!
//! A profile fixes both the kind of a component and the exact files generated
//! for it. Changing what a profile generates requires a new profile version.

/// A pure state machine: owned state, typed inputs and outputs, and a total
/// transition function implemented by the implementer.
pub const TRANSITION_CORE_V0: &str = "transition-core/v0";

/// A container that owns no behavioral state and composes its children through
/// explicit FIFO mailboxes and approved message edges.
pub const COMPOSITE_V0: &str = "composite/v0";

/// Every profile this generator version implements.
pub const SUPPORTED: &[&str] = &[COMPOSITE_V0, TRANSITION_CORE_V0];
