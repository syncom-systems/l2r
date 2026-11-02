//! The generation lock: what L2R wrote last time, so that later runs can tell
//! untouched generated files from edited ones.

use serde::{Deserialize, Serialize};

use l2r_model::digest;
use l2r_plan::{Ownership, Plan};

/// Lock format identifier.
pub const LOCK_FORMAT: &str = "l2r.lock/v0";

/// File name of the lock inside the output directory.
pub const LOCK_FILE: &str = "l2r.lock.json";

/// The record of one generation run. Its content depends only on the plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    /// Format identifier.
    pub format: String,
    /// Digest of the plan that was written.
    pub plan_digest: String,
    /// Generator identity, `name version`.
    pub generator: String,
    /// Every planned file.
    pub files: Vec<LockEntry>,
}

/// One file recorded in the lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockEntry {
    /// Path relative to the output directory.
    pub path: String,
    /// Ownership class at generation time.
    pub ownership: LockOwnership,
    /// Digest of the content L2R wrote; only for generated files, because
    /// implementation files are expected to change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

/// Ownership as recorded in the lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockOwnership {
    /// L2R-owned.
    Generated,
    /// Implementer-owned.
    Implementation,
}

impl From<Ownership> for LockOwnership {
    fn from(ownership: Ownership) -> Self {
        match ownership {
            Ownership::Generated => Self::Generated,
            Ownership::Implementation => Self::Implementation,
        }
    }
}

impl Lock {
    /// The lock describing `plan` once written.
    pub fn for_plan(plan: &Plan) -> Self {
        Self {
            format: LOCK_FORMAT.into(),
            plan_digest: plan.digest(),
            generator: format!("{} {}", plan.generator.name, plan.generator.version),
            files: plan
                .artifacts
                .iter()
                .map(|a| LockEntry {
                    path: a.path.clone(),
                    ownership: a.ownership.into(),
                    digest: (a.ownership == Ownership::Generated)
                        .then(|| digest::sha256(a.content.as_bytes())),
                })
                .collect(),
        }
    }

    /// Finds the entry for `path`.
    pub fn entry(&self, path: &str) -> Option<&LockEntry> {
        self.files.iter().find(|e| e.path == path)
    }

    /// Stable JSON encoding.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("lock serializes") + "\n"
    }

    /// Parses a lock, rejecting unknown formats.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let lock: Lock = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if lock.format != LOCK_FORMAT {
            return Err(format!("unsupported lock format `{}`", lock.format));
        }
        Ok(lock)
    }
}
