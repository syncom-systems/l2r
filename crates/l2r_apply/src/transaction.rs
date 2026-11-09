//! Applying an approved plan, and finishing an interrupted application.
//!
//! A generation runs in four phases:
//!
//! 1. **Assess**: compute every action; any conflict refuses the run, and nothing
//!    is written.
//! 2. **Stage**: write every new file's content to an exclusively created
//!    temporary beside its target and sync it. A failure removes the temporaries;
//!    the tree's files are unchanged (new, empty parent directories may remain).
//! 3. **Journal**: create `l2r.journal.json` exclusively. From here on the run can
//!    be finished by [`recover`]. If the journal already exists, another
//!    generation is pending or running, and this one stops.
//! 4. **Commit**: one step per file, each re-checking the target's digest
//!    immediately before acting. Creates never replace an existing file.
//!    Replacements are atomic renames, and deletions remove only an unmodified
//!    generated file. Then the tree is checked to be in the plan's state, the
//!    new lock is written atomically and the journal removed.
//!
//! **Authority.** Every step must be one that generating the approved plan from
//! the recorded prior lock could produce ([`Journal::authorize`]); this holds for
//! the journal `apply` writes and is re-checked for the journal `recover` reads.
//! Recovery runs steps only while that same prior lock is on disk, so it has no
//! authority beyond what generation from the same recorded state has. A lock is
//! written only after a final-state check confirms the tree holds every planned
//! generated file with its planned content, every planned implementation file,
//! and none of the prior lock's no-longer-planned generated files. Omitting steps
//! from a journal therefore cannot produce a lock that misstates the tree.
//!
//! Guarantee: after an interruption at any point, every file is either in its
//! old or its new state, and the journal records which plan was being applied.
//! [`recover`] with the same approved plan completes it. A file whose content
//! matches neither expected state is never overwritten or deleted: the run stops
//! and reports it. Implementation files are only ever created, never replaced or
//! removed. Files and directories are synced (`fsync`) at each step, but
//! durability across power loss depends on the filesystem honouring it.

use std::fmt;
use std::path::Path;

use l2r_model::digest;
use l2r_plan::{Ownership, Plan, declares_generated};

use crate::approval::{self, Approval, ApprovalError, Change, destructive_changes};
use crate::assess::{
    ActionKind, Assessment, Conflict, RecordedLock, assess_recorded, assess_tree, conflict,
    read_recorded_lock,
};
use crate::journal::{JOURNAL_FILE, JOURNAL_FORMAT, Journal, Step, StepKind};
use crate::lock::{LOCK_FILE, Lock, LockOwnership};
use crate::tree::{Occupant, TempNames, Tree, TreeError};

/// Why a plan was not (completely) written.
#[derive(Debug)]
pub enum ApplyError {
    /// The approval does not cover this plan.
    Approval(ApprovalError),
    /// The tree is not in a state the plan can be applied to. Nothing was written.
    Conflicts(Vec<Conflict>),
    /// Staging failed. The tree's files are unchanged.
    Staging(TreeError),
    /// The run stopped part-way through committing. The journal is in place;
    /// resolve the reported problem and run `l2r recover`.
    Interrupted(TreeError),
    /// There is no journal to recover from.
    NothingToRecover,
    /// The journal is unreadable, tampered with, or names a different plan.
    Journal(String),
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Approval(e) => write!(f, "{e}"),
            Self::Conflicts(conflicts) => {
                writeln!(
                    f,
                    "refusing to write: {} conflict(s); nothing was written",
                    conflicts.len()
                )?;
                for c in conflicts {
                    writeln!(f, "  {}: {}", c.path, c.reason)?;
                }
                Ok(())
            }
            Self::Staging(e) => write!(f, "staging failed, no file was changed: {e}"),
            Self::Interrupted(e) => write!(
                f,
                "generation stopped part-way: {e}\nthe journal records the plan; resolve this and run `l2r recover`"
            ),
            Self::NothingToRecover => write!(f, "no interrupted generation to recover"),
            Self::Journal(message) => write!(f, "cannot recover: {message}"),
        }
    }
}

impl std::error::Error for ApplyError {}

/// The result of a successful write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The assessment that was carried out.
    pub assessment: Assessment,
}

/// Writes `plan` under `root`, provided `approval` covers it and no conflict exists.
pub fn apply(plan: &Plan, approval: &Approval, root: &Path) -> Result<Report, ApplyError> {
    approval::verify(approval, plan).map_err(ApplyError::Approval)?;
    if let Some(tree) =
        Tree::open(root).map_err(|e| ApplyError::Conflicts(vec![conflict(e.path, e.reason)]))?
    {
        let mut assessment = assess_tree(plan, &tree);
        assessment
            .conflicts
            .extend(unauthorized(&assessment, approval));
        if !assessment.conflicts.is_empty() {
            return Err(ApplyError::Conflicts(assessment.conflicts));
        }
    }
    let tree = Tree::open_or_create(root).map_err(ApplyError::Staging)?;
    // Re-assess on the tree we will write to, so the steps describe exactly it.
    let (mut assessment, recorded) = assess_recorded(plan, &tree);
    assessment
        .conflicts
        .extend(unauthorized(&assessment, approval));
    if !assessment.conflicts.is_empty() {
        return Err(ApplyError::Conflicts(assessment.conflicts));
    }
    let prior = recorded.as_ref().map(|r| &r.lock);
    let names = TempNames::new();
    let journal = journal_for(plan, &assessment, recorded.as_ref(), &names);
    // One definition of a permitted transition, for writing and for recovery.
    journal
        .authorize(plan, prior, approval, &|_| false)
        .map_err(|e| ApplyError::Journal(format!("generator defect: {e}")))?;
    stage(plan, &tree, &journal.steps).map_err(ApplyError::Staging)?;
    let created = tree
        .create_state(JOURNAL_FILE, &names.next(), journal.to_json().as_bytes())
        .map_err(ApplyError::Staging);
    match created {
        Ok(true) => {}
        Ok(false) => {
            discard_temps(&tree, &journal.steps);
            return Err(ApplyError::Conflicts(vec![conflict(
                JOURNAL_FILE,
                "another generation started concurrently; nothing was written",
            )]));
        }
        Err(e) => {
            discard_temps(&tree, &journal.steps);
            return Err(e);
        }
    }
    commit(&tree, plan, &journal, prior, &names).map_err(ApplyError::Interrupted)?;
    Ok(Report { assessment })
}

/// Finishes an interrupted generation of `plan` under `root`.
///
/// The journal must name exactly `plan`, which must still be approved. What
/// happens next depends on the lock on disk:
///
/// - It is the prior lock the journal records (same bytes): every step must be
///   authorized by the plan and that lock, missing or damaged staged content is
///   re-staged from the plan itself, and the steps are committed.
/// - It is already the plan's own lock (the interruption came after it was
///   written): no step runs; the tree is only checked to be in the plan's state.
/// - Anything else: refused, because the recorded state the steps were derived
///   from is gone.
///
/// Either way the final-state check runs before the journal is removed.
pub fn recover(plan: &Plan, approval: &Approval, root: &Path) -> Result<(), ApplyError> {
    approval::verify(approval, plan).map_err(ApplyError::Approval)?;
    let tree = Tree::open(root)
        .map_err(|e| ApplyError::Journal(e.to_string()))?
        .ok_or(ApplyError::NothingToRecover)?;
    let journal = read_journal(&tree)?;
    check_journal_names_plan(&journal, plan)?;
    let on_disk = read_recorded_lock(&tree).map_err(|c| ApplyError::Journal(c.reason))?;
    let names = TempNames::new();
    if on_disk.as_ref().map(|r| &r.digest) == journal.prior_lock.as_ref() {
        let prior = on_disk.as_ref().map(|r| &r.lock);
        let done = |step: &Step| match (step.kind, tree.read(&step.path)) {
            (StepKind::Replace, Ok(Occupant::File(b))) => Some(digest::sha256(&b)) == step.after,
            (StepKind::Delete, Ok(Occupant::Absent)) => true,
            _ => false,
        };
        journal
            .authorize(plan, prior, approval, &done)
            .map_err(|e| {
                ApplyError::Journal(format!(
                    "{e}; the plan, the prior lock and the approval record do not authorize it"
                ))
            })?;
        let mut steps = journal.steps.clone();
        restage_missing(plan, &tree, &mut steps, &names).map_err(ApplyError::Interrupted)?;
        let journal = Journal { steps, ..journal };
        commit(&tree, plan, &journal, prior, &names).map_err(ApplyError::Interrupted)
    } else if on_disk.as_ref().is_some_and(|r| r.lock == journal.lock) {
        verify_applied(&tree, plan, None).map_err(ApplyError::Interrupted)?;
        discard_temps(&tree, &journal.steps);
        tree.remove_state(JOURNAL_FILE)
            .map_err(ApplyError::Interrupted)
    } else {
        Err(ApplyError::Journal(
            "the lock changed after the generation started, so the recorded state its steps were \
             derived from is gone; restore that lock or resolve by hand"
                .into(),
        ))
    }
}

/// Replacements and deletions the approval record does not name. The lock may
/// say a file is L2R's to replace or remove; only the reviewed record can
/// authorize doing it to this content.
fn unauthorized(assessment: &Assessment, approval: &Approval) -> Vec<Conflict> {
    let granted = destructive_changes(assessment);
    granted
        .iter()
        .filter(|c| !approval.authorizes(&c.path, c.change, &c.before))
        .map(|c| {
            let what = match c.change {
                Change::Replace => "replacing",
                Change::Delete => "deleting",
            };
            conflict(
                c.path.as_str(),
                format!(
                    "{what} this file ({}) is not authorized by the approval record; review the preview and \
                     approve again so the record names this change",
                    c.before
                ),
            )
        })
        .collect()
}

/// True when a journal is present under `root`.
pub fn pending(root: &Path) -> bool {
    matches!(Tree::open(root), Ok(Some(tree)) if matches!(tree.read_state(JOURNAL_FILE), Ok(Occupant::File(_))))
}

fn read_journal(tree: &Tree) -> Result<Journal, ApplyError> {
    match tree.read_state(JOURNAL_FILE) {
        Ok(Occupant::Absent) => Err(ApplyError::NothingToRecover),
        Ok(Occupant::File(bytes)) => {
            let text = String::from_utf8(bytes)
                .map_err(|_| ApplyError::Journal("journal is not UTF-8".into()))?;
            Journal::from_json(&text).map_err(ApplyError::Journal)
        }
        Err(e) => Err(ApplyError::Journal(e.to_string())),
    }
}

/// The journal must name the approved plan and the lock that plan produces.
/// Its steps are authorized separately, against the recorded prior lock.
fn check_journal_names_plan(journal: &Journal, plan: &Plan) -> Result<(), ApplyError> {
    if journal.plan_digest != plan.digest() {
        return Err(ApplyError::Journal(format!(
            "the journal is for plan {}, but the approved plan is {}; restore the model that produced it",
            journal.plan_digest,
            plan.digest()
        )));
    }
    if journal.lock != Lock::for_plan(plan) {
        return Err(ApplyError::Journal(
            "the journal's lock does not match the plan".into(),
        ));
    }
    Ok(())
}

fn journal_for(
    plan: &Plan,
    assessment: &Assessment,
    prior: Option<&RecordedLock>,
    names: &TempNames,
) -> Journal {
    let mut writes = Vec::new();
    let mut deletes = Vec::new();
    for action in &assessment.actions {
        let after = || {
            plan.artifact(action.path.as_str())
                .map(|a| digest::sha256(a.content.as_bytes()))
        };
        let before = action.current.as_deref().map(digest::sha256);
        let step = |kind, before, after, temp| Step {
            path: action.path.clone(),
            ownership: action.ownership,
            kind,
            before,
            after,
            temp,
        };
        match action.kind {
            ActionKind::Create => {
                writes.push(step(StepKind::Create, None, after(), Some(names.next())))
            }
            ActionKind::Update => {
                writes.push(step(StepKind::Replace, before, after(), Some(names.next())))
            }
            ActionKind::Delete => deletes.push(step(StepKind::Delete, before, None, None)),
            ActionKind::Unchanged | ActionKind::Keep | ActionKind::Orphan => {}
        }
    }
    writes.extend(deletes);
    Journal {
        format: JOURNAL_FORMAT.into(),
        plan_digest: plan.digest(),
        prior_lock: prior.map(|r| r.digest.clone()),
        steps: writes,
        lock: Lock::for_plan(plan),
    }
}

fn planned_bytes<'p>(plan: &'p Plan, step: &Step) -> &'p [u8] {
    plan.artifact(step.path.as_str())
        .map(|a| a.content.as_bytes())
        .expect("write steps are planned")
}

fn stage(plan: &Plan, tree: &Tree, steps: &[Step]) -> Result<(), TreeError> {
    for (index, step) in steps.iter().enumerate() {
        if let Some(temp) = &step.temp
            && let Err(e) = crate::fault::staging_checkpoint(index + 1)
                .and_then(|()| tree.stage(&step.path, temp, planned_bytes(plan, step)))
        {
            discard_temps(tree, &steps[..=index]);
            return Err(e);
        }
    }
    Ok(())
}

fn restage_missing(
    plan: &Plan,
    tree: &Tree,
    steps: &mut [Step],
    names: &TempNames,
) -> Result<(), TreeError> {
    for step in steps.iter_mut() {
        let Some(temp) = step.temp.clone() else {
            continue;
        };
        let intact = matches!(tree.read_temp(&step.path, &temp)?, Occupant::File(b) if Some(digest::sha256(&b)) == step.after);
        let already_done = matches!(tree.read(&step.path)?, Occupant::File(b) if Some(digest::sha256(&b)) == step.after);
        if intact || already_done {
            continue;
        }
        tree.remove_temp(&step.path, &temp)?;
        let fresh = names.next();
        tree.stage(&step.path, &fresh, planned_bytes(plan, step))?;
        step.temp = Some(fresh);
    }
    Ok(())
}

fn discard_temps(tree: &Tree, steps: &[Step]) {
    for step in steps {
        if let Some(temp) = &step.temp {
            let _ = tree.remove_temp(&step.path, temp);
        }
    }
}

fn digest_of(occupant: &Occupant) -> Option<String> {
    match occupant {
        Occupant::Absent => None,
        Occupant::File(bytes) => Some(digest::sha256(bytes)),
    }
}

fn mismatch(step: &Step, found: &Occupant) -> TreeError {
    let state = match found {
        Occupant::Absent => "is missing".to_string(),
        Occupant::File(_) => "was changed by someone else".to_string(),
    };
    TreeError {
        path: step.path.to_string(),
        reason: format!("{state} during generation; it was left untouched"),
    }
}

/// Commits one step. Idempotent: a step whose target already has its final
/// state is complete.
fn commit_step(tree: &Tree, step: &Step) -> Result<(), TreeError> {
    let found = tree.read(&step.path)?;
    let current = digest_of(&found);
    match step.kind {
        StepKind::Delete => {
            let marked = matches!(&found, Occupant::File(b) if declares_generated(b));
            if current.is_some() && current == step.before && marked {
                tree.remove(&step.path)?;
            } else if current.is_some() {
                return Err(mismatch(step, &found));
            }
        }
        StepKind::Create | StepKind::Replace => {
            let temp = step
                .temp
                .as_ref()
                .expect("validated write step has a temporary");
            if current.is_some() && current == step.after {
                tree.remove_temp(&step.path, temp)?;
            } else if current.is_none() {
                if !tree.place_new(&step.path, temp)? {
                    return commit_step(tree, step); // someone created it meanwhile; re-examine
                }
            } else if step.kind == StepKind::Replace && current == step.before {
                tree.replace(&step.path, temp)?;
            } else if step.ownership == LockOwnership::Implementation {
                // The implementer's file wins over L2R's stub.
                tree.remove_temp(&step.path, temp)?;
            } else {
                return Err(mismatch(step, &found));
            }
        }
    }
    Ok(())
}

/// The final-state check: the tree holds every planned generated file with its
/// planned content and every planned implementation file, and none of `prior`'s
/// generated files that the plan no longer contains. Only then may a lock
/// claiming the plan be written.
fn verify_applied(tree: &Tree, plan: &Plan, prior: Option<&Lock>) -> Result<(), TreeError> {
    let unmet = |path: &str, what: &str| TreeError {
        path: path.to_string(),
        reason: format!("{what}, so the plan is not fully applied; the lock was not written"),
    };
    for artifact in &plan.artifacts {
        match (artifact.ownership, tree.read(&artifact.path)?) {
            (_, Occupant::Absent) => return Err(unmet(artifact.path.as_str(), "is missing")),
            (Ownership::Generated, Occupant::File(bytes))
                if bytes != artifact.content.as_bytes() =>
            {
                return Err(unmet(
                    artifact.path.as_str(),
                    "does not have its planned content",
                ));
            }
            _ => {}
        }
    }
    for entry in prior.iter().flat_map(|l| &l.files) {
        let planned = plan.artifact(entry.path.as_str()).is_some();
        if !planned
            && entry.ownership == LockOwnership::Generated
            && tree.read(&entry.path)? != Occupant::Absent
        {
            return Err(unmet(
                entry.path.as_str(),
                "is no longer planned but still present",
            ));
        }
    }
    Ok(())
}

fn commit(
    tree: &Tree,
    plan: &Plan,
    journal: &Journal,
    prior: Option<&Lock>,
    names: &TempNames,
) -> Result<(), TreeError> {
    crate::fault::checkpoint(0)?;
    for (index, step) in journal.steps.iter().enumerate() {
        commit_step(tree, step)?;
        crate::fault::checkpoint(index + 1)?;
    }
    verify_applied(tree, plan, prior)?;
    tree.write_state(LOCK_FILE, &names.next(), journal.lock.to_json().as_bytes())?;
    crate::fault::checkpoint(journal.steps.len() + 1)?;
    tree.remove_state(JOURNAL_FILE)
}
