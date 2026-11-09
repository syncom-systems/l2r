//! Deciding, before any write, what applying a plan would do to the tree.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use l2r_model::{RelPath, digest};
use l2r_plan::{Artifact, Ownership, Plan, declares_generated};

use crate::journal::JOURNAL_FILE;
use crate::lock::{LOCK_FILE, Lock, LockEntry, LockOwnership};
use crate::tree::{Occupant, Tree};

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
    pub path: RelPath,
    /// What will happen.
    pub kind: ActionKind,
    /// Ownership of the file.
    pub ownership: LockOwnership,
    /// Current content, for updates and deletes.
    pub current: Option<Vec<u8>>,
}

/// A reason the plan cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Path relative to the output directory (or a state file name).
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

pub(crate) fn conflict(path: impl Into<String>, reason: impl Into<String>) -> Conflict {
    Conflict {
        path: path.into(),
        reason: reason.into(),
    }
}

/// The lock as found on disk, with the digest of its exact bytes.
#[derive(Debug, Clone)]
pub(crate) struct RecordedLock {
    pub(crate) lock: Lock,
    pub(crate) digest: String,
}

/// Reads and validates the lock. A malformed lock is a conflict, not a reset.
pub fn read_lock(tree: &Tree) -> Result<Option<Lock>, Conflict> {
    read_recorded_lock(tree).map(|r| r.map(|r| r.lock))
}

pub(crate) fn read_recorded_lock(tree: &Tree) -> Result<Option<RecordedLock>, Conflict> {
    match tree
        .read_state(LOCK_FILE)
        .map_err(|e| conflict(LOCK_FILE, e.reason))?
    {
        Occupant::Absent => Ok(None),
        Occupant::File(bytes) => {
            let digest = digest::sha256(&bytes);
            let text =
                String::from_utf8(bytes).map_err(|_| conflict(LOCK_FILE, "lock is not UTF-8"))?;
            let lock = Lock::from_json(&text).map_err(|e| {
                conflict(
                    LOCK_FILE,
                    format!("lock is invalid ({e}); nothing it lists will be touched"),
                )
            })?;
            Ok(Some(RecordedLock { lock, digest }))
        }
    }
}

/// Reports a pending journal as a conflict.
pub fn journal_conflict(tree: &Tree) -> Option<Conflict> {
    match tree.read_state(JOURNAL_FILE) {
        Ok(Occupant::Absent) => None,
        Ok(Occupant::File(_)) => Some(conflict(
            JOURNAL_FILE,
            "an interrupted or concurrent generation is pending; run `l2r recover` with the plan it names",
        )),
        Err(e) => Some(conflict(JOURNAL_FILE, e.reason)),
    }
}

/// Computes what applying `plan` to `root` would do, without writing.
pub fn assess(plan: &Plan, root: &Path) -> Assessment {
    match Tree::open(root) {
        Ok(Some(tree)) => assess_tree(plan, &tree),
        Ok(None) => Assessment {
            actions: plan
                .artifacts
                .iter()
                .map(|a| action(&a.path, ActionKind::Create, a.ownership.into(), None))
                .collect(),
            conflicts: vec![],
        },
        Err(e) => Assessment {
            actions: vec![],
            conflicts: vec![conflict(e.path, e.reason)],
        },
    }
}

/// Computes what applying `plan` to an open tree would do.
pub fn assess_tree(plan: &Plan, tree: &Tree) -> Assessment {
    assess_recorded(plan, tree).0
}

/// [`assess_tree`], also returning the lock the assessment was made against.
pub(crate) fn assess_recorded(plan: &Plan, tree: &Tree) -> (Assessment, Option<RecordedLock>) {
    let mut actions = Vec::new();
    let mut conflicts: Vec<Conflict> = journal_conflict(tree).into_iter().collect();
    let recorded = read_recorded_lock(tree).unwrap_or_else(|c| {
        conflicts.push(c);
        None
    });
    let lock = recorded.as_ref().map(|r| r.lock.clone());
    for artifact in &plan.artifacts {
        let entry = lock.as_ref().and_then(|l| l.entry(&artifact.path));
        let outcome = tree
            .read(&artifact.path)
            .map_err(|e| e.reason)
            .and_then(|occupant| planned_action(artifact, entry, occupant));
        match outcome {
            Ok(a) => actions.push(a),
            Err(reason) => conflicts.push(conflict(artifact.path.as_str(), reason)),
        }
    }
    if let Some(lock) = &lock {
        unplanned(plan, lock, tree, &mut actions, &mut conflicts);
        moved_implementations(plan, lock, &actions, &mut conflicts);
    }
    actions.sort_by(|a, b| a.path.cmp(&b.path));
    conflicts.sort_by(|a, b| a.path.cmp(&b.path));
    (Assessment { actions, conflicts }, recorded)
}

fn unplanned(
    plan: &Plan,
    lock: &Lock,
    tree: &Tree,
    actions: &mut Vec<Action>,
    conflicts: &mut Vec<Conflict>,
) {
    let planned: BTreeMap<String, &RelPath> = plan
        .artifacts
        .iter()
        .map(|a| (a.path.case_key(), &a.path))
        .collect();
    let exact: BTreeSet<&RelPath> = plan.artifacts.iter().map(|a| &a.path).collect();
    for entry in lock.files.iter().filter(|e| !exact.contains(&e.path)) {
        if let Some(other) = planned.get(&entry.path.case_key()) {
            conflicts.push(conflict(
                entry.path.as_str(),
                format!("differs from planned `{other}` only in letter case; resolve by hand"),
            ));
            continue;
        }
        let outcome = tree
            .read(&entry.path)
            .map_err(|e| e.reason)
            .and_then(|o| unplanned_action(entry, o));
        match outcome {
            Ok(Some(a)) => actions.push(a),
            Ok(None) => {}
            Err(reason) => conflicts.push(conflict(entry.path.as_str(), reason)),
        }
    }
}

/// Creating a fresh implementation stub while an implementation file of the same
/// name is being orphaned would silently start the implementer over: it is what a
/// renamed module, a component moved to a new parent, or a renamed component looks
/// like. Generation fails closed and names both paths; the human moves the file
/// (or deletes an obsolete one) and generates again.
fn moved_implementations(
    plan: &Plan,
    lock: &Lock,
    actions: &[Action],
    conflicts: &mut Vec<Conflict>,
) {
    let orphans: Vec<&Action> = actions
        .iter()
        .filter(|a| a.kind == ActionKind::Orphan)
        .collect();
    for created in actions
        .iter()
        .filter(|a| a.kind == ActionKind::Create && a.ownership == LockOwnership::Implementation)
    {
        let name = created.path.split().1;
        let owner = plan
            .artifact(created.path.as_str())
            .and_then(|art| art.components.first().cloned())
            .unwrap_or_default();
        let candidates: Vec<&&Action> = orphans
            .iter()
            .filter(|o| o.path.split().1 == name)
            .collect();
        let same_owner = candidates
            .iter()
            .find(|o| lock.entry(&o.path).is_some_and(|e| e.component == owner));
        let reason = match (same_owner, candidates.first()) {
            (Some(orphan), _) => format!(
                "the implementation of `{owner}` moves from `{}`; move that file here (e.g. `git mv`) and generate \
                 again",
                orphan.path
            ),
            (None, Some(orphan)) => format!(
                "a new implementation stub would be created while `{}` (component `{}`) is orphaned; if the \
                 component was renamed, move that file here, otherwise delete it, then generate again",
                orphan.path,
                lock.entry(&orphan.path)
                    .map(|e| e.component.as_str())
                    .unwrap_or("?")
            ),
            (None, None) => continue,
        };
        conflicts.push(conflict(created.path.as_str(), reason));
    }
}

fn action(
    path: &RelPath,
    kind: ActionKind,
    ownership: LockOwnership,
    current: Option<Vec<u8>>,
) -> Action {
    Action {
        path: path.clone(),
        kind,
        ownership,
        current,
    }
}

fn planned_action(
    artifact: &Artifact,
    entry: Option<&LockEntry>,
    occupant: Occupant,
) -> Result<Action, String> {
    let path = &artifact.path;
    let ownership: LockOwnership = artifact.ownership.into();
    if let Some(entry) = entry
        && entry.ownership != ownership
    {
        return Err(format!(
            "ownership changes from {:?} to {:?}; migrate this file by hand and regenerate",
            entry.ownership, artifact.ownership
        ));
    }
    match (artifact.ownership, occupant) {
        (_, Occupant::Absent) => Ok(action(path, ActionKind::Create, ownership, None)),
        (Ownership::Implementation, Occupant::File(_)) => {
            Ok(action(path, ActionKind::Keep, ownership, None))
        }
        (Ownership::Generated, Occupant::File(bytes)) => {
            if bytes == artifact.content.as_bytes() {
                return Ok(action(path, ActionKind::Unchanged, ownership, None));
            }
            match entry.and_then(|e| e.digest.as_deref()) {
                Some(d) if d == digest::sha256(&bytes) => Ok(action(path, ActionKind::Update, ownership, Some(bytes))),
                Some(_) => Err("generated file was edited after generation; move the change into the model or \
                                Lean, or restore the file"
                    .into()),
                None => Err("a file L2R did not generate is in the way".into()),
            }
        }
    }
}

fn unplanned_action(entry: &LockEntry, occupant: Occupant) -> Result<Option<Action>, String> {
    let Occupant::File(bytes) = occupant else {
        return Ok(None);
    };
    match entry.ownership {
        LockOwnership::Implementation => Ok(Some(action(
            &entry.path,
            ActionKind::Orphan,
            entry.ownership,
            None,
        ))),
        // The lock is untrusted: a forged entry naming someone's file with its
        // digest must not turn it into a deletion. L2R deletes only files that
        // declare themselves generated, as everything it generates does.
        LockOwnership::Generated if !declares_generated(&bytes) => Err(
            "the lock records this file as generated, but it lacks L2R's generated-file marker; it is \
             not deleted, resolve by hand"
                .into(),
        ),
        LockOwnership::Generated
            if entry.digest.as_deref() == Some(digest::sha256(&bytes).as_str()) =>
        {
            Ok(Some(action(
                &entry.path,
                ActionKind::Delete,
                entry.ownership,
                Some(bytes),
            )))
        }
        LockOwnership::Generated => {
            Err("generated file is no longer planned but was edited; resolve by hand".into())
        }
    }
}
