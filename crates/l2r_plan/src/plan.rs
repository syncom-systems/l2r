//! The generation plan: every file L2R would write, with its exact content,
//! ownership and sources, bound together by one digest.

use serde::Serialize;

use l2r_model::{RelPath, digest};

/// Plan format identifier.
pub const PLAN_FORMAT: &str = "l2r.plan/v0";

/// Identity of the generator producing a plan. Part of the plan digest, so a
/// different generator version invalidates prior approvals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeneratorIdentity {
    /// Tool name.
    pub name: String,
    /// Tool version.
    pub version: String,
}

impl GeneratorIdentity {
    /// The identity of this build of L2R.
    pub fn current() -> Self {
        Self {
            name: "l2r".into(),
            version: crate::GENERATOR_VERSION.into(),
        }
    }
}

/// Who owns a file after it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    /// L2R owns the file. It is rewritten from the model and must not be edited.
    Generated,
    /// The implementer owns the file. L2R creates it once and never changes it.
    Implementation,
}

/// One planned file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Path relative to the output directory.
    pub path: RelPath,
    /// Ownership class.
    pub ownership: Ownership,
    /// Profile that produced the file.
    pub profile: String,
    /// Why the file exists.
    pub purpose: String,
    /// Components the file is derived from; the first is the owning component.
    pub components: Vec<String>,
    /// Lean declarations the file refers to.
    pub lean: Vec<String>,
    /// Exact content. For implementation files, the initial stub.
    pub content: String,
}

/// A complete, deterministic generation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Generator that produced the plan.
    pub generator: GeneratorIdentity,
    /// Digest of the canonical model.
    pub model_digest: String,
    /// Digest of the canonical Lean inventory.
    pub inventory_digest: String,
    /// Profiles used, sorted.
    pub profiles: Vec<String>,
    /// Output directory, relative to the model file.
    pub output_dir: String,
    /// Planned files, sorted by path.
    pub artifacts: Vec<Artifact>,
}

/// Everything a plan digest covers, in canonical order.
#[derive(Debug, Serialize)]
pub struct PlanSummary<'a> {
    /// Plan format identifier.
    pub format: &'static str,
    /// Generator identity.
    pub generator: &'a GeneratorIdentity,
    /// Model digest.
    pub model_digest: &'a str,
    /// Inventory digest.
    pub inventory_digest: &'a str,
    /// Profiles used.
    pub profiles: &'a [String],
    /// Output directory.
    pub output_dir: &'a str,
    /// One entry per planned file.
    pub artifacts: Vec<ArtifactSummary<'a>>,
}

/// The digest-relevant facts of one planned file.
#[derive(Debug, Serialize)]
pub struct ArtifactSummary<'a> {
    /// Path relative to the output directory.
    pub path: &'a str,
    /// Ownership class.
    pub ownership: Ownership,
    /// Profile.
    pub profile: &'a str,
    /// Purpose.
    pub purpose: &'a str,
    /// Source components.
    pub components: &'a [String],
    /// Lean references.
    pub lean: &'a [String],
    /// Digest of the exact content.
    pub content_digest: String,
}

impl Plan {
    /// The canonical summary that the plan digest is computed over.
    pub fn summary(&self) -> PlanSummary<'_> {
        PlanSummary {
            format: PLAN_FORMAT,
            generator: &self.generator,
            model_digest: &self.model_digest,
            inventory_digest: &self.inventory_digest,
            profiles: &self.profiles,
            output_dir: &self.output_dir,
            artifacts: self
                .artifacts
                .iter()
                .map(|a| ArtifactSummary {
                    path: a.path.as_str(),
                    ownership: a.ownership,
                    profile: &a.profile,
                    purpose: &a.purpose,
                    components: &a.components,
                    lean: &a.lean,
                    content_digest: digest::sha256(a.content.as_bytes()),
                })
                .collect(),
        }
    }

    /// The digest an approval must name.
    pub fn digest(&self) -> String {
        digest::digest_of(&self.summary())
    }

    /// Finds a planned file by path.
    pub fn artifact(&self, path: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.path.as_str() == path)
    }
}
