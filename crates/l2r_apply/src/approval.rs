//! The approval record: a human statement that one exact plan may be written.
//!
//! An approval names the plan digest the architect saw in the preview. It is
//! never computed on the architect's behalf: [`approve`] refuses a digest that
//! differs from the current plan, and [`verify`] refuses an approval whose
//! digest no longer matches, listing which inputs changed.

use std::fmt;

use serde::{Deserialize, Serialize};

use l2r_plan::Plan;

/// Approval format identifier.
pub const APPROVAL_FORMAT: &str = "l2r.approval/v0";

/// The statement every approval records.
pub const STATEMENT: &str = "I reviewed the generation plan with this digest, including the content of every planned file, and approve writing it.";

/// A recorded approval. It is evidence for review, not proof of identity:
/// `approved_by` is whatever the approver typed, and the protection comes from
/// committing the record through normal code review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    /// Format identifier.
    pub format: String,
    /// The approved plan digest.
    pub plan_digest: String,
    /// Model digest at approval time.
    pub model_digest: String,
    /// Inventory digest at approval time.
    pub inventory_digest: String,
    /// Generator identity at approval time, `name version`.
    pub generator: String,
    /// Profiles in the approved plan.
    pub profiles: Vec<String>,
    /// Number of planned files.
    pub artifact_count: usize,
    /// Who approved, as stated by the approver.
    pub approved_by: String,
    /// What was approved.
    pub statement: String,
}

/// Why an approval was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    /// The supplied digest is not the digest of the current plan.
    DigestMismatch {
        /// Digest the approver supplied.
        supplied: String,
        /// Digest of the current plan.
        actual: String,
    },
    /// The approval was for a different plan; `reasons` lists what changed.
    Stale {
        /// Human-readable reasons.
        reasons: Vec<String>,
    },
    /// No approver name was given.
    MissingApprover,
    /// The approval record is not in a supported format.
    Unsupported(String),
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DigestMismatch { supplied, actual } => write!(
                f,
                "the supplied digest {supplied} is not the current plan ({actual}); review the current preview and approve its digest"
            ),
            Self::Stale { reasons } => write!(f, "the approval is stale: {}", reasons.join("; ")),
            Self::MissingApprover => write!(f, "an approver name is required"),
            Self::Unsupported(format) => write!(f, "unsupported approval format `{format}`"),
        }
    }
}

impl std::error::Error for ApprovalError {}

fn generator_label(plan: &Plan) -> String {
    format!("{} {}", plan.generator.name, plan.generator.version)
}

/// Records approval of `plan`, provided `supplied_digest` is its digest.
pub fn approve(
    plan: &Plan,
    supplied_digest: &str,
    approved_by: &str,
) -> Result<Approval, ApprovalError> {
    if approved_by.trim().is_empty() || approved_by.chars().any(char::is_control) {
        return Err(ApprovalError::MissingApprover);
    }
    let actual = plan.digest();
    if supplied_digest != actual {
        return Err(ApprovalError::DigestMismatch {
            supplied: supplied_digest.to_string(),
            actual,
        });
    }
    Ok(Approval {
        format: APPROVAL_FORMAT.into(),
        plan_digest: actual,
        model_digest: plan.model_digest.clone(),
        inventory_digest: plan.inventory_digest.clone(),
        generator: generator_label(plan),
        profiles: plan.profiles.clone(),
        artifact_count: plan.artifacts.len(),
        approved_by: approved_by.trim().to_string(),
        statement: STATEMENT.into(),
    })
}

/// Confirms that `approval` approves exactly `plan`.
pub fn verify(approval: &Approval, plan: &Plan) -> Result<(), ApprovalError> {
    if approval.format != APPROVAL_FORMAT {
        return Err(ApprovalError::Unsupported(approval.format.clone()));
    }
    if approval.plan_digest == plan.digest() {
        return Ok(());
    }
    let mut reasons = Vec::new();
    if approval.model_digest != plan.model_digest {
        reasons.push("the architecture model changed".to_string());
    }
    if approval.inventory_digest != plan.inventory_digest {
        reasons.push("the Lean inventory changed".to_string());
    }
    if approval.generator != generator_label(plan) {
        reasons.push(format!(
            "the generator changed from `{}` to `{}`",
            approval.generator,
            generator_label(plan)
        ));
    }
    if approval.profiles != plan.profiles {
        reasons.push("the set of generation profiles changed".to_string());
    }
    if reasons.is_empty() {
        reasons.push("the planned files differ from the approved plan".to_string());
    }
    Err(ApprovalError::Stale { reasons })
}

/// Encodes an approval as stable, human-readable JSON.
pub fn to_json(approval: &Approval) -> String {
    serde_json::to_string_pretty(approval).expect("approval serializes") + "\n"
}

/// Parses an approval record.
pub fn from_json(text: &str) -> Result<Approval, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}
