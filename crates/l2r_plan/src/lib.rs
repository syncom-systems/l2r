//! Deterministic generation planning.
//!
//! [`plan`] turns a [`Validated`] model into a [`Plan`]: the complete, ordered
//! list of files L2R would write, with exact content, ownership class and
//! sources. Planning is pure: it reads nothing from disk, uses no clock or
//! randomness, and iterates only over sorted collections, so identical inputs
//! give byte-identical plans and the same plan digest.

mod layout;
mod lean_composition;
mod plan;
mod profiles;
mod text;

pub use lean_composition::{LEAN_COMPOSITION_FORMAT, lean_composition, lean_composition_path};
pub use text::declares_generated;

pub use plan::{
    Artifact, ArtifactSummary, GeneratorIdentity, Ownership, PLAN_FORMAT, Plan, PlanSummary,
};

use std::collections::BTreeSet;
use std::fmt;

use l2r_model::RelPath;
use l2r_model::{digest, profiles as profile_ids};
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
    let model = validated.model();
    let dirs = layout::component_dirs(model);
    let root = model
        .components
        .iter()
        .find(|c| c.parent.is_none())
        .ok_or_else(|| PlanError("no root component".into()))?;

    let mut artifacts = profiles::crate_root::artifacts(model, &root.rust_module, &root.id)?;
    let mut used_profiles = BTreeSet::new();
    for component in &model.components {
        let dir = &dirs[&component.id];
        used_profiles.insert(component.profile.clone());
        match component.profile.as_str() {
            profile_ids::TRANSITION_CORE_V0 => {
                let machine = validated.machine(&component.id).ok_or_else(|| {
                    PlanError(format!("machine `{}` was not resolved", component.id))
                })?;
                artifacts.extend(profiles::transition_core::artifacts(
                    component, machine, dir,
                )?);
            }
            profile_ids::COMPOSITE_V0 => {
                artifacts.extend(profiles::composite::artifacts(model, component, dir)?)
            }
            other => {
                return Err(PlanError(format!(
                    "profile `{other}` has no implementation"
                )));
            }
        }
    }
    used_profiles.insert(profiles::crate_root::PROFILE.to_string());

    artifacts.sort_by(|a, b| a.path.cmp(&b.path));
    check_paths(&artifacts)?;

    Ok(Plan {
        generator: generator.clone(),
        model_digest: digest::digest_of(model),
        inventory_digest: digest::digest_of(validated.inventory()),
        profiles: used_profiles.into_iter().collect(),
        output_dir: model.system.output_dir.clone(),
        artifacts,
    })
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
