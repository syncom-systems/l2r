//! The generation lock: what L2R wrote last time, so that later runs can tell
//! untouched generated files from edited ones.
//!
//! The lock is read back from the output tree, so it is untrusted input. Its
//! paths are [`RelPath`]s (validated during deserialization), and
//! [`Lock::from_json`] rejects duplicate or case-colliding paths and malformed
//! digests before any entry is used.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use l2r_model::{RelPath, digest};
use l2r_plan::{Ownership, Plan};

/// Lock format identifier.
pub const LOCK_FORMAT: &str = "l2r.lock/v0";

/// File name of the lock at the root of the output tree.
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
    pub path: RelPath,
    /// Ownership class at generation time.
    pub ownership: LockOwnership,
    /// The component that owns the file.
    pub component: String,
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

/// True for `sha256:` followed by 64 lowercase hex digits.
pub fn is_digest(text: &str) -> bool {
    text.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64 && hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    })
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
                    component: a.components.first().cloned().unwrap_or_default(),
                    digest: (a.ownership == Ownership::Generated)
                        .then(|| digest::sha256(a.content.as_bytes())),
                })
                .collect(),
        }
    }

    /// Finds the entry for `path`.
    pub fn entry(&self, path: &RelPath) -> Option<&LockEntry> {
        self.files.iter().find(|e| &e.path == path)
    }

    /// Stable JSON encoding.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("lock serializes") + "\n"
    }

    /// Parses and validates a lock read from disk.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let lock: Lock = serde_json::from_str(text).map_err(|e| e.to_string())?;
        lock.validate()?;
        Ok(lock)
    }

    /// Structural checks beyond what deserialization enforces.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != LOCK_FORMAT {
            return Err(format!("unsupported lock format `{}`", self.format));
        }
        if !is_digest(&self.plan_digest) {
            return Err("malformed plan digest".into());
        }
        let mut seen = BTreeSet::new();
        for entry in &self.files {
            let id_ok = !entry.component.is_empty()
                && entry.component.len() <= 64
                && entry
                    .component
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !id_ok {
                return Err(format!("`{}` names an invalid component", entry.path));
            }
            if !seen.insert(entry.path.case_key()) {
                return Err(format!(
                    "`{}` is listed twice (case-insensitively)",
                    entry.path
                ));
            }
            match (entry.ownership, &entry.digest) {
                (LockOwnership::Generated, Some(d)) if is_digest(d) => {}
                (LockOwnership::Generated, _) => {
                    return Err(format!("`{}` has no valid digest", entry.path));
                }
                (LockOwnership::Implementation, None) => {}
                (LockOwnership::Implementation, Some(_)) => {
                    return Err(format!(
                        "implementation file `{}` must not carry a digest",
                        entry.path
                    ));
                }
            }
        }
        Ok(())
    }
}
