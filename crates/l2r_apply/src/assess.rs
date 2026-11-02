//! Deciding, before any write, what applying a plan would do to the disk.

use std::collections::BTreeSet;
use std::path::Path;

use l2r_model::digest;
use l2r_plan::{Artifact, Ownership, Plan};

use crate::fs_guard::{self, Occupant};
use crate::lock::{LOCK_FILE, Lock, LockEntry, LockOwnership};

/// What will happen to one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// Write a file that does not exist yet.
    Create,
    /// Replace an unmodified generated file with new content.
    Update,
    /// The generated file already has the planned content.
    Unchanged,
    /// The implementation file exists and is left alone.
    Keep,
    /// Remove an unmodified generated file that is no longer planned.
    Delete,
    /// An implementation file is no longer planned; it is left in place for a human to resolve.
    Orphan,
}

/// One planned disk action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// Path relative to the output directory.
    pub path: String,
    /// What will happen.
    pub kind: ActionKind,
    /// Current content, when it matters for a preview (updates and deletes).
    pub current: Option<Vec<u8>>,
}

/// A reason the plan cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Path relative to the output directory.
    pub path: String,
    /// Explanation.
    pub reason: String,
}

/// The full effect of applying a plan, computed without writing anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    /// Actions, sorted by path.
    pub actions: Vec<Action>,
    /// Conflicts; when non-empty, nothing may be written.
    pub conflicts: Vec<Conflict>,
}

impl Assessment {
    /// Actions of the given kind.
    pub fn of_kind(&self, kind: ActionKind) -> impl Iterator<Item = &Action> {
        self.actions.iter().filter(move |a| a.kind == kind)
    }
}

/// Reads the previous lock, if any. A malformed lock is a conflict, not a reset.
pub fn read_lock(root: &Path) -> Result<Option<Lock>, Conflict> {
    let conflict = |reason| Conflict {
        path: LOCK_FILE.into(),
        reason,
    };
    match fs_guard::inspect(root, LOCK_FILE).map_err(conflict)? {
        Occupant::Absent => Ok(None),
        Occupant::File(bytes) => {
            let text =
                String::from_utf8(bytes).map_err(|_| conflict("lock is not UTF-8".into()))?;
            Lock::from_json(&text)
                .map(Some)
                .map_err(|e| conflict(format!("lock is malformed: {e}")))
        }
    }
}

/// Computes what applying `plan` to `root` would do.
pub fn assess(plan: &Plan, root: &Path) -> Assessment {
    let mut actions = Vec::new();
    let mut conflicts = Vec::new();
    let lock = match read_lock(root) {
        Ok(lock) => lock,
        Err(conflict) => {
            conflicts.push(conflict);
            None
        }
    };
    for artifact in &plan.artifacts {
        let entry = lock.as_ref().and_then(|l| l.entry(&artifact.path));
        match fs_guard::inspect(root, &artifact.path) {
            Err(reason) => conflicts.push(Conflict {
                path: artifact.path.clone(),
                reason,
            }),
            Ok(occupant) => match planned_action(artifact, entry, occupant) {
                Ok(action) => actions.push(action),
                Err(reason) => conflicts.push(Conflict {
                    path: artifact.path.clone(),
                    reason,
                }),
            },
        }
    }
    if let Some(lock) = &lock {
        let planned: BTreeSet<&str> = plan.artifacts.iter().map(|a| a.path.as_str()).collect();
        for entry in lock
            .files
            .iter()
            .filter(|e| !planned.contains(e.path.as_str()))
        {
            match unplanned_action(root, entry) {
                Ok(Some(action)) => actions.push(action),
                Ok(None) => {}
                Err(reason) => conflicts.push(Conflict {
                    path: entry.path.clone(),
                    reason,
                }),
            }
        }
    }
    actions.sort_by(|a, b| a.path.cmp(&b.path));
    conflicts.sort_by(|a, b| a.path.cmp(&b.path));
    Assessment { actions, conflicts }
}

fn action(path: &str, kind: ActionKind, current: Option<Vec<u8>>) -> Action {
    Action {
        path: path.to_string(),
        kind,
        current,
    }
}

fn planned_action(
    artifact: &Artifact,
    entry: Option<&LockEntry>,
    occupant: Occupant,
) -> Result<Action, String> {
    let path = artifact.path.as_str();
    if let Some(entry) = entry
        && entry.ownership != artifact.ownership.into()
    {
        return Err(format!(
            "ownership changes from {:?} to {:?}; migrate this file by hand and regenerate",
            entry.ownership, artifact.ownership
        ));
    }
    match (artifact.ownership, occupant) {
        (_, Occupant::Absent) => Ok(action(path, ActionKind::Create, None)),
        (Ownership::Implementation, Occupant::File(_)) => Ok(action(path, ActionKind::Keep, None)),
        (Ownership::Generated, Occupant::File(bytes)) => {
            if bytes == artifact.content.as_bytes() {
                return Ok(action(path, ActionKind::Unchanged, None));
            }
            let recorded = entry.and_then(|e| e.digest.as_deref());
            match recorded {
                Some(d) if d == digest::sha256(&bytes) => Ok(action(path, ActionKind::Update, Some(bytes))),
                Some(_) => Err("generated file was edited after generation; move the change into the model or Lean, or restore the file".into()),
                None => Err("a file L2R did not generate is in the way".into()),
            }
        }
    }
}

fn unplanned_action(root: &Path, entry: &LockEntry) -> Result<Option<Action>, String> {
    match fs_guard::inspect(root, &entry.path)? {
        Occupant::Absent => Ok(None),
        Occupant::File(bytes) => match entry.ownership {
            LockOwnership::Implementation => {
                Ok(Some(action(&entry.path, ActionKind::Orphan, None)))
            }
            LockOwnership::Generated => {
                if entry.digest.as_deref() == Some(digest::sha256(&bytes).as_str()) {
                    Ok(Some(action(&entry.path, ActionKind::Delete, Some(bytes))))
                } else {
                    Err(
                        "generated file is no longer planned but was edited; resolve by hand"
                            .into(),
                    )
                }
            }
        },
    }
}
