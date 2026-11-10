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

/// A pure state machine whose Lean state, input and output types may carry
/// data (structures, constructors with fields, fixed-width integers). Its Rust
/// types are generated once per Lean type in the crate's `types` module. Model
/// schema `v2`.
pub const TRANSITION_CORE_V1: &str = "transition-core/v1";

/// A composite that may be nested and has typed input and output ports. Its
/// behaviour is that of the flat network of the machines below it
/// (`l2r.async/v1`); the root composite generates the scheduler. Model schema
/// `v2`.
pub const COMPOSITE_V2: &str = "composite/v2";

/// Every profile this generator version implements.
pub const SUPPORTED: &[&str] = &[
    COMPOSITE_V0,
    COMPOSITE_V1,
    COMPOSITE_V2,
    TRANSITION_CORE_V0,
    TRANSITION_CORE_V1,
];
