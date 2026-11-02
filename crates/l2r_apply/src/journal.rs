//! The generation journal: the record of a generation in progress.
//!
//! While a journal exists, the tree may be part-way between two plans. The
//! journal names the plan being applied and every step with its expected
//! before/after digests, so that [`crate::recover`] can finish the same
//! approved plan after an interruption, verifying each step as it goes. Its
//! exclusive creation also keeps two generations from interleaving.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use l2r_model::RelPath;

use crate::lock::{Lock, LockOwnership, is_digest};
use crate::tree::TempName;

/// Journal format identifier.
pub const JOURNAL_FORMAT: &str = "l2r.journal/v0";

/// File name of the journal at the root of the output tree.
pub const JOURNAL_FILE: &str = "l2r.journal.json";

/// A generation in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    /// Format identifier.
    pub format: String,
    /// Digest of the plan being applied.
    pub plan_digest: String,
    /// Steps, in commit order.
    pub steps: Vec<Step>,
    /// The lock to write once every step is committed.
    pub lock: Lock,
}

/// One change to one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Target path.
    pub path: RelPath,
    /// Ownership of the target.
    pub ownership: LockOwnership,
    /// What happens.
    pub kind: StepKind,
    /// Digest the target has before the step (`replace`, `delete`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Digest the target has after the step (`create`, `replace`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Staged content beside the target (`create`, `replace`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp: Option<TempName>,
}

/// Kinds of step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// Place a new file; never replaces an existing one.
    Create,
    /// Replace a generated file that still has its `before` digest.
    Replace,
    /// Remove a generated file that still has its `before` digest.
    Delete,
}

impl Journal {
    /// Stable JSON encoding.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("journal serializes") + "\n"
    }

    /// Parses and validates a journal read from disk.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let journal: Journal = serde_json::from_str(text).map_err(|e| e.to_string())?;
        journal.validate()?;
        Ok(journal)
    }

    fn validate(&self) -> Result<(), String> {
        if self.format != JOURNAL_FORMAT {
            return Err(format!("unsupported journal format `{}`", self.format));
        }
        if !is_digest(&self.plan_digest) || self.lock.plan_digest != self.plan_digest {
            return Err("journal and its lock name different plans".into());
        }
        self.lock.validate()?;
        let mut seen = BTreeSet::new();
        for step in &self.steps {
            if !seen.insert(step.path.case_key()) {
                return Err(format!("`{}` has more than one step", step.path));
            }
            step.validate()?;
        }
        Ok(())
    }
}

impl Step {
    fn validate(&self) -> Result<(), String> {
        let digest_ok = |d: &Option<String>| d.as_deref().is_some_and(is_digest);
        let shape_ok = match self.kind {
            StepKind::Create => {
                digest_ok(&self.after) && self.before.is_none() && self.temp.is_some()
            }
            StepKind::Replace => {
                digest_ok(&self.before) && digest_ok(&self.after) && self.temp.is_some()
            }
            StepKind::Delete => {
                digest_ok(&self.before) && self.after.is_none() && self.temp.is_none()
            }
        };
        if !shape_ok {
            return Err(format!("step for `{}` is malformed", self.path));
        }
        // Only L2R-owned files may ever be replaced or deleted.
        if self.kind != StepKind::Create && self.ownership != LockOwnership::Generated {
            return Err(format!(
                "step would {:?} implementation file `{}`",
                self.kind, self.path
            ));
        }
        Ok(())
    }
}
