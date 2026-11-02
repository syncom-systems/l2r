//! The approval record: evidence that one exact plan was reviewed.
//!
//! A record names the plan digest shown in the preview. It is never computed on
//! the reviewer's behalf: [`approve`] refuses a digest that differs from the
//! current plan, and [`verify`] refuses a record whose digest no longer matches,
//! listing which inputs changed.
//!
//! What a record is **not**: proof of who reviewed, or authorization. Anyone or
//! any agent able to run `l2r approve` can create one, and `recorded_by` is free
//! text. Authorization has to come from outside L2R, e.g. a required human review
//! of changes to approval files under the repository's branch rules. L2R binds
//! the write to the recorded digest; the repository's governance decides whose
//! records count.

use std::fmt;

use serde::{Deserialize, Serialize};

use l2r_plan::Plan;

/// Approval format identifier.
pub const APPROVAL_FORMAT: &str = "l2r.approval/v0";

/// The statement every record carries.
pub const STATEMENT: &str = "The recorder states that the generation plan with this digest, including the content of every planned file, was reviewed and may be written. This record is not proof of identity or authorization.";

/// A recorded approval of one exact plan. Evidence, not authorization.
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
    /// Who recorded the approval, as typed by them (unverified).
    pub recorded_by: String,
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
            Self::MissingApprover => write!(f, "a recorder name is required"),
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
    recorded_by: &str,
) -> Result<Approval, ApprovalError> {
    if recorded_by.trim().is_empty() || recorded_by.chars().any(char::is_control) {
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
        recorded_by: recorded_by.trim().to_string(),
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

/// Writes a record to `path` atomically: an exclusively created temporary in the
/// same directory, renamed over `path`. A symbolic link planted at `path` is
/// replaced, never followed.
pub fn write_record(path: &std::path::Path, approval: &Approval) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let written = file
        .write_all(to_json(approval).as_bytes())
        .and_then(|()| file.sync_all());
    if let Err(e) = written.and_then(|()| std::fs::rename(&temporary, path)) {
        let _ = std::fs::remove_file(&temporary);
        return Err(e);
    }
    Ok(())
}
