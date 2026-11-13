//! Deterministic generation planning.
//!
//! [`plan`] turns a [`Validated`] model into a [`Plan`]: the complete, ordered
//! list of files L2R would write, with exact content, ownership class and
//! sources. Planning is pure: it reads nothing from disk, uses no clock or
//! randomness, and iterates only over sorted collections, so identical inputs
//! give byte-identical plans and the same plan digest.

mod conformance;
mod lean_composition;
mod lean_composition_v2;
mod plan;
mod profiles;
mod text;
mod v2;

pub use conformance::{CONFORMANCE_FORMAT, ConformanceSide, conformance};
pub use lean_composition::{
    LEAN_COMPOSITION_FORMAT, LEAN_COMPOSITION_FORMAT_V1, SEMANTICS_VERSION, SEMANTICS_VERSION_V1,
    lean_composition, lean_composition_path, semantics_digest, semantics_digest_v1,
};
pub use lean_composition_v2::{
    LEAN_COMPOSITION_FORMAT_V2, PORTS_SCHEDULER_VERSION, ports_scheduler_digest,
};
pub use text::declares_generated;

pub use plan::{
    Artifact, ArtifactSummary, GeneratorIdentity, Ownership, PLAN_FORMAT, Plan, PlanSummary,
};

use std::collections::BTreeSet;
use std::fmt;

use l2r_model::RelPath;
use l2r_model::{Profile, Schema, digest};
use l2r_validate::Validated;

/// Version of this generator, bound into every plan digest.
pub const GENERATOR_VERSION: &str = "0.1.0";

/// Planning failed despite a validated model; this indicates a generator defect
/// and is reported rather than resolved by guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError(pub String);

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "planning failed: {}", self.0)
    }
}

impl std::error::Error for PlanError {}

/// Plans every file for `validated`, as produced by `generator`.
pub fn plan(validated: &Validated, generator: &GeneratorIdentity) -> Result<Plan, PlanError> {
    // Each schema has its own planner; a new schema is a compile error here.
    let (mut artifacts, used_profiles) = match validated.schema() {
        Schema::V0 | Schema::V1 => v0_v1_artifacts(validated)?,
        Schema::V2 => v2::artifacts(validated)?,
    };
    artifacts.sort_by(|a, b| a.path.cmp(&b.path));
    check_paths(&artifacts)?;
    let model = validated.model();
    Ok(Plan {
        generator: generator.clone(),
        model_digest: digest::digest_of(model),
        inventory_digest: digest::digest_of(validated.inventory()),
        profiles: used_profiles.into_iter().collect(),
        output_dir: model.system.output_dir.clone(),
        artifacts,
    })
}

/// Plans every file of a `v0` or `v1` model and names the profiles used.
fn v0_v1_artifacts(validated: &Validated) -> Result<(Vec<Artifact>, BTreeSet<String>), PlanError> {
    let model = validated.model();
    let dirs = profiles::layout::component_dirs(model);
    let root = model
        .components
        .iter()
        .find(|c| c.parent.is_none())
        .ok_or_else(|| PlanError("no root component".into()))?;
    let mut artifacts = profiles::crate_root::artifacts(model, &root.rust_module, &root.id)?;
    let mut used_profiles = BTreeSet::from([profiles::crate_root::PROFILE.to_string()]);
    for component in &model.components {
        let dir = &dirs[&component.id];
        used_profiles.insert(component.profile.clone());
        artifacts.extend(v0_v1_component(validated, component, dir)?);
    }
    Ok((artifacts, used_profiles))
}

/// The files of one component of a `v0` or `v1` model, planned by its profile.
fn v0_v1_component(
    validated: &Validated,
    component: &l2r_model::Component,
    dir: &str,
) -> Result<Vec<Artifact>, PlanError> {
    match validated.profile(&component.id) {
        Profile::TransitionCoreV0 => {
            let machine = validated
                .machine(&component.id)
                .ok_or_else(|| PlanError(format!("machine `{}` was not resolved", component.id)))?;
            profiles::transition_core::artifacts(component, machine, dir)
        }
        Profile::CompositeV0 => {
            let names = validated.composite_v0_names(&component.id).ok_or_else(|| {
                PlanError(format!("composite `{}` has no checked names", component.id))
            })?;
            profiles::composite::artifacts(validated.model(), component, names, dir)
        }
        Profile::CompositeV1 => profiles::composite_v1::artifacts(validated, component, dir),
        other @ (Profile::TransitionCoreV1 | Profile::CompositeV2) => Err(PlanError(format!(
            "profile `{}` has no implementation",
            other.id()
        ))),
    }
}

/// A planned path, validated like every other path that reaches the filesystem.
pub(crate) fn planned_path(path: String) -> Result<RelPath, PlanError> {
    RelPath::parse(&path).map_err(|e| PlanError(e.to_string()))
}

/// Defense in depth: planned paths are unique, also on case-insensitive
/// filesystems, and every generated file declares itself as generated, which is
/// what allows the writer to delete it once it is no longer planned.
fn check_paths(artifacts: &[Artifact]) -> Result<(), PlanError> {
    let mut seen = BTreeSet::new();
    for artifact in artifacts {
        if artifact.ownership == Ownership::Generated
            && !declares_generated(artifact.content.as_bytes())
        {
            return Err(PlanError(format!(
                "generated artifact `{}` lacks the generated-file marker",
                artifact.path
            )));
        }
        if !seen.insert(artifact.path.case_key()) {
            return Err(PlanError(format!(
                "two artifacts planned at `{}`",
                artifact.path
            )));
        }
    }
    Ok(())
}
