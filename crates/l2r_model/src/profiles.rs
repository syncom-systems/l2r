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

/// A container whose children have bounded mailboxes (capacity is part of the
/// semantics, `l2r.async/v1`), with typed input ports and a scheduler whose
/// deliveries are enabled only when every receiver has room. `composite/v0` is
/// unchanged and remains available.
pub const COMPOSITE_V1: &str = "composite/v1";

/// Every profile this generator version implements.
pub const SUPPORTED: &[&str] = &[COMPOSITE_V0, COMPOSITE_V1, TRANSITION_CORE_V0];
