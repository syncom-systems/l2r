//! Identities of the Lean side of a derivation: the format identifiers of the
//! derived files, and the versions and digests of the Lean semantics and
//! scheduler models they are wired to.
//!
//! Every deriver and the conformance harness record these in their headers.
//! They sit apart from the derivers so that each deriver, and the dispatch
//! between them, uses them without the derivers using each other (audit F15).

use l2r_model::digest;

/// Format identifier of the derived file for `composite/v0` composites.
pub const LEAN_COMPOSITION_FORMAT: &str = "l2r.lean-composition/v0";

/// Format identifier of the derived file for `composite/v1` composites: the
/// network additionally records mailbox capacities and typed ports, and is wired
/// to `l2r.async/v1`. `v0` output is unchanged.
pub const LEAN_COMPOSITION_FORMAT_V1: &str = "l2r.lean-composition/v1";

/// Identity of the `l2r.async/v1` semantics (`lean/L2R/Async1.lean`).
pub const SEMANTICS_VERSION_V1: &str = "l2r.async/v1";

const SEMANTICS_SOURCE_V1: &str = include_str!("../../../lean/L2R/Async1.lean");

/// Digest of the `l2r.async/v1` semantics source this build derives against.
pub fn semantics_digest_v1() -> String {
    digest::sha256(SEMANTICS_SOURCE_V1.as_bytes())
}

/// Identity of the semantics the derived network is wired to. It names the
/// version of `lean/L2R/Async.lean`; a behavioural change there is a new
/// version with a new file, never an edit under the same name.
pub const SEMANTICS_VERSION: &str = "l2r.async/v0";

/// The exact text of the semantics file the derived network imports. Its digest
/// is recorded in every derivation, so editing the semantics invalidates every
/// derived file (and the freshness test) until it is derived again and reviewed.
const SEMANTICS_SOURCE: &str = include_str!("../../../lean/L2R/Async.lean");

/// Digest of the semantics source this build of L2R derives against.
pub fn semantics_digest() -> String {
    digest::sha256(SEMANTICS_SOURCE.as_bytes())
}

/// Format identifier of the derived file for `v2` (hierarchical) models.
pub const LEAN_COMPOSITION_FORMAT_V2: &str = "l2r.lean-composition/v2";

/// Identity of the Lean model of the `composite/v2` scheduler
/// (`lean/L2R/Async1Ports.lean`).
pub const PORTS_SCHEDULER_VERSION: &str = "l2r.async1-ports/v1";

const PORTS_SCHEDULER_SOURCE: &str = include_str!("../../../lean/L2R/Async1Ports.lean");

/// Digest of the scheduler model source this build derives against.
pub fn ports_scheduler_digest() -> String {
    digest::sha256(PORTS_SCHEDULER_SOURCE.as_bytes())
}
