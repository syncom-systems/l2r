//! The generation journal: the record of a generation in progress.
//!
//! While a journal exists, the tree may be part-way between two plans. The
//! journal names the plan being applied, the lock it was assessed against (the
//! *prior lock*), and every step with its expected before/after digests, so that
//! [`crate::recover`] can finish the same approved plan after an interruption.
//! Its exclusive creation also keeps two generations from interleaving.
//!
//! The journal lives in the output tree and is therefore untrusted. It confers
//! no authority of its own: [`Journal::authorize`] admits a step only if
//! generating the approved plan from the recorded prior lock could have produced
//! it. Steps it omits are caught by the final-state check made before any lock
//! is written (see [`crate::transaction`]).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use l2r_model::{RelPath, digest};
use l2r_plan::Plan;

use crate::approval::{Approval, Change};
use crate::lock::{Lock, LockOwnership, is_digest};
use crate::tree::TempName;

/// Journal format identifier. `v1` added `prior_lock`; a `v0` journal is refused,
/// because it cannot show which recorded state its steps were derived from.
pub const JOURNAL_FORMAT: &str = "l2r.journal/v1";

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
    /// Digest of the exact bytes of the lock the generation was assessed against,
    /// or `null` when there was none. Recovery runs steps only while that same
    /// lock is still in place.
    pub prior_lock: Option<String>,
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
        if self.prior_lock.as_deref().is_some_and(|d| !is_digest(d)) {
            return Err("malformed prior-lock digest".into());
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

    /// Admits the journal only if every step is a transition that generating
    /// `plan` from `prior` (the recorded prior lock) could produce. The plan and
    /// the prior lock are the only sources of authority; the journal's own claims
    /// about ownership and digests are checked against them, never believed.
    ///
    /// A replacement or deletion additionally needs the approval record's entry
    /// for that content, unless `done` says the step has already happened (its
    /// target already has the planned content, or is already gone): re-running
    /// it destroys nothing, and a record made after an interruption cannot list
    /// it, because it was made from a partly applied tree.
    pub fn authorize(
        &self,
        plan: &Plan,
        prior: Option<&Lock>,
        approval: &Approval,
        done: &dyn Fn(&Step) -> bool,
    ) -> Result<(), String> {
        for step in &self.steps {
            step.authorize(plan, prior)
                .and_then(|()| {
                    if done(step) {
                        Ok(())
                    } else {
                        step.authorized_by(approval)
                    }
                })
                .map_err(|why| format!("step for `{}` {why}", step.path))?;
        }
        Ok(())
    }
}

impl Step {
    /// The permitted transitions, per path:
    ///
    /// | Path                                    | Permitted step                                    |
    /// | --------------------------------------- | ------------------------------------------------- |
    /// | planned, any ownership                  | `create` of the planned content, plan's ownership |
    /// | planned, generated, recorded generated  | `replace` from the recorded digest to the plan's  |
    /// | not planned, recorded generated         | `delete` of the recorded digest                   |
    /// | anything else                           | nothing                                           |
    ///
    /// Ownership must agree with the plan (planned paths) and with the prior
    /// lock (recorded paths), exactly as an ownership change is a conflict for
    /// generation.
    fn authorize(&self, plan: &Plan, prior: Option<&Lock>) -> Result<(), &'static str> {
        let artifact = plan.artifact(self.path.as_str());
        let recorded = prior.and_then(|lock| lock.entry(&self.path));
        if recorded.is_some_and(|e| e.ownership != self.ownership) {
            return Err("claims an ownership the prior lock does not record");
        }
        let Some(artifact) = artifact else {
            // Not planned: only the deletion of a recorded generated file.
            let collides = plan
                .artifacts
                .iter()
                .any(|a| a.path.case_key() == self.path.case_key());
            return match (self.kind, recorded) {
                _ if collides => Err("differs from a planned path only in letter case"),
                (StepKind::Delete, Some(e))
                    if e.ownership == LockOwnership::Generated && e.digest == self.before =>
                {
                    Ok(())
                }
                (StepKind::Delete, _) => Err(
                    "deletes a file the prior lock does not record as generated with that content",
                ),
                _ => Err("writes a file the plan does not contain"),
            };
        };
        if LockOwnership::from(artifact.ownership) != self.ownership {
            return Err("claims an ownership the plan does not give it");
        }
        let planned = Some(digest::sha256(artifact.content.as_bytes()));
        match self.kind {
            StepKind::Delete => Err("deletes a planned file"),
            _ if self.after != planned => Err("does not match the plan's content"),
            StepKind::Create => Ok(()),
            StepKind::Replace => match recorded {
                Some(e) if e.ownership == LockOwnership::Generated && e.digest == self.before => {
                    Ok(())
                }
                _ => Err("replaces content the prior lock does not record as generated"),
            },
        }
    }
    /// Replacing or deleting needs the approval record's explicit entry for this
    /// path and this exact prior content; the lock alone never suffices.
    fn authorized_by(&self, approval: &Approval) -> Result<(), &'static str> {
        let change = match self.kind {
            StepKind::Create => return Ok(()),
            StepKind::Replace => Change::Replace,
            StepKind::Delete => Change::Delete,
        };
        match &self.before {
            Some(before) if approval.authorizes(&self.path, change, before) => Ok(()),
            _ => Err("is not authorized by the approval record for that file content"),
        }
    }

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
