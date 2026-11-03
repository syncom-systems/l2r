//! Hostile output trees and tampered state files. Every "victim" lives in a
//! scratch directory beside the output tree; nothing outside the test's
//! scratch area is ever touched.

use std::fs;
use std::path::{Path, PathBuf};

use l2r_apply::{Approval, apply, approve, assess, check, lock};
use l2r_model::{Model, digest};
use l2r_plan::{GeneratorIdentity, Plan, plan};
use l2r_testing::{example_inventory, example_model, scratch_dir};
use l2r_validate::validate;

fn plan_for(model: &Model) -> Plan {
    plan(
        &validate(model, &example_inventory()).expect("valid"),
        &GeneratorIdentity::current(),
    )
    .expect("plans")
}

fn approved(plan: &Plan, root: &Path) -> Approval {
    // The record is made once, from the tree as it is before generation, as a
    // reviewer's preview would show it; later calls for the same tree and plan
    // return that same record, as a committed approval file would.
    static RECORDS: std::sync::Mutex<Vec<(PathBuf, String, Approval)>> =
        std::sync::Mutex::new(Vec::new());
    let mut records = RECORDS.lock().unwrap();
    let key = (root.to_path_buf(), plan.digest());
    if let Some((_, _, a)) = records.iter().find(|(r, d, _)| (r, d) == (&key.0, &key.1)) {
        return a.clone();
    }
    let record = approve(plan, &plan.digest(), "test", &assess(plan, root)).expect("approves");
    records.push((key.0, key.1, record.clone()));
    record
}

/// An output tree `<scratch>/out` holding the generated example, and a victim
/// file `<scratch>/victim.txt` outside it.
fn fixture(label: &str) -> (PathBuf, PathBuf) {
    let base = scratch_dir(label);
    let root = base.join("out");
    let plan = plan_for(&example_model());
    apply(&plan, &approved(&plan, &root), &root).expect("initial generation");
    let victim = base.join("victim.txt");
    fs::write(&victim, "outside the output tree\n").unwrap();
    (root, victim)
}

/// Tampers with the raw lock JSON, as an attacker with write access to the
/// output tree could; the typed API cannot even represent these entries.
fn add_lock_entry(root: &Path, path: &str, digest: Option<String>) {
    let text = fs::read_to_string(root.join(lock::LOCK_FILE)).unwrap();
    let mut raw: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut entry =
        serde_json::json!({ "path": path, "ownership": "generated", "component": "signal" });
    if let Some(d) = digest {
        entry["digest"] = serde_json::Value::String(d);
    }
    raw["files"].as_array_mut().unwrap().push(entry);
    fs::write(
        root.join(lock::LOCK_FILE),
        serde_json::to_string_pretty(&raw).unwrap(),
    )
    .unwrap();
}

fn victim_digest(victim: &Path) -> Option<String> {
    Some(digest::sha256(&fs::read(victim).unwrap()))
}

#[test]
fn a_lock_entry_with_dot_dot_cannot_delete_outside_the_tree() {
    let (root, victim) = fixture("lock-dotdot");
    add_lock_entry(&root, "../victim.txt", victim_digest(&victim));
    let plan = plan_for(&example_model());
    let result = apply(&plan, &approved(&plan, &root), &root);
    assert!(victim.exists(), "the victim outside the tree was deleted");
    assert!(
        result.is_err(),
        "a lock with an escaping path must be refused"
    );
}

#[test]
fn a_lock_entry_with_an_absolute_path_cannot_delete_outside_the_tree() {
    let (root, victim) = fixture("lock-absolute");
    add_lock_entry(&root, &victim.display().to_string(), victim_digest(&victim));
    let plan = plan_for(&example_model());
    let result = apply(&plan, &approved(&plan, &root), &root);
    assert!(victim.exists(), "the victim outside the tree was deleted");
    assert!(
        result.is_err(),
        "a lock with an absolute path must be refused"
    );
}

#[test]
fn check_does_not_probe_files_outside_the_tree() {
    let (root, victim) = fixture("lock-probe");
    add_lock_entry(&root, "../victim.txt", None);
    let plan = plan_for(&example_model());
    let findings = check(&example_model(), &plan, &root);
    // The only acceptable report is that the lock itself is invalid; nothing may
    // reveal whether `../victim.txt` exists or what it contains.
    assert!(
        findings.iter().all(|f| f.path == lock::LOCK_FILE),
        "check reported on a path outside the tree: {findings:?}"
    );
    assert!(victim.exists());
}

// The temporary-file symlink attack is tested in `faults.rs`
// (`symlinks_planted_at_the_exact_temporary_names_are_refused`), where the
// fault build makes temporary names predictable. This file reproduced the
// original defect against the old fixed names (`.mod.rs.l2r-tmp`), which the
// current code no longer uses.

#[test]
fn a_lock_entry_naming_the_lock_itself_is_refused() {
    let (root, _victim) = fixture("lock-self");
    let own = digest::sha256(&fs::read(root.join(lock::LOCK_FILE)).unwrap());
    add_lock_entry(&root, lock::LOCK_FILE, Some(own));
    let plan = plan_for(&example_model());
    assert!(
        !assess(&plan, &root).conflicts.is_empty(),
        "the lock may not describe itself"
    );
}

#[test]
fn a_forged_lock_entry_cannot_make_generate_delete_a_file_l2r_did_not_write() {
    // The lock is untrusted. A forged "generated" entry with the right digest
    // must not turn a file L2R never wrote into a deletion.
    let (root, _victim) = fixture("lock-forged-delete");
    let notes = root.join("src/notes.rs");
    fs::write(&notes, "// the implementer's own notes\n").unwrap();
    add_lock_entry(
        &root,
        "src/notes.rs",
        Some(digest::sha256(&fs::read(&notes).unwrap())),
    );
    let plan = plan_for(&example_model());
    let result = apply(&plan, &approved(&plan, &root), &root);
    assert!(notes.exists(), "generate deleted a file L2R never wrote");
    // Refused during assessment, before anything is staged or journaled.
    assert!(
        matches!(result, Err(l2r_apply::ApplyError::Conflicts(_))),
        "{result:?}"
    );
    assert!(!l2r_apply::pending(&root));
}

// ---------------------------------------------------------------------------
// The lock is untrusted evidence. It cannot authorize destruction: replacing or
// deleting an existing generated file needs the reviewed approval record to
// name that file with its exact content digest.
// ---------------------------------------------------------------------------

const MARKED: &str =
    "// @generated by L2R (forged by an attacker with write access to the tree)\nkeep me\n";

/// An approval for `plan` as a reviewer would record it from `root` right now.
fn approved_after_preview(plan: &Plan, root: &Path) -> Approval {
    approve(plan, &plan.digest(), "reviewer", &assess(plan, root)).expect("approves")
}

fn rewrite_lock_digest(root: &Path, path: &str, digest: &str) {
    let lock_path = root.join(lock::LOCK_FILE);
    let mut raw: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
    for entry in raw["files"].as_array_mut().unwrap() {
        if entry["path"] == path {
            entry["digest"] = serde_json::Value::String(digest.into());
        }
    }
    fs::write(lock_path, serde_json::to_string_pretty(&raw).unwrap()).unwrap();
}

fn refusals(result: Result<l2r_apply::Report, l2r_apply::ApplyError>) -> String {
    match result {
        Err(l2r_apply::ApplyError::Conflicts(c)) => c
            .iter()
            .map(|c| format!("{}: {}", c.path, c.reason))
            .collect::<Vec<_>>()
            .join("\n"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_forged_lock_and_marker_cannot_authorize_a_deletion() {
    // Everything an attacker controls in the output tree is forged consistently:
    // a file with the generated-file marker, and a lock entry with its digest.
    // Before the approval record carried destructive authority, this deleted it.
    let (root, _victim) = fixture("forged-marker-delete");
    let plan = plan_for(&example_model());
    let approval = approved(&plan, &root); // recorded before the forgery
    let planted = root.join("src/planted.rs");
    fs::write(&planted, MARKED).unwrap();
    add_lock_entry(
        &root,
        "src/planted.rs",
        Some(digest::sha256(MARKED.as_bytes())),
    );
    let why = refusals(apply(&plan, &approval, &root));
    assert!(planted.exists(), "a forged lock authorized a deletion");
    assert!(
        why.contains("src/planted.rs") && why.contains("not authorized"),
        "{why}"
    );
    assert!(!l2r_apply::pending(&root));
}

#[test]
fn a_reviewer_who_sees_the_deletion_in_the_preview_can_authorize_it() {
    // The same state is deleted once a reviewed record names it: the authority
    // comes from the record, which the output tree cannot edit.
    let (root, _victim) = fixture("reviewed-delete");
    let plan = plan_for(&example_model());
    let planted = root.join("src/planted.rs");
    fs::write(&planted, MARKED).unwrap();
    add_lock_entry(
        &root,
        "src/planted.rs",
        Some(digest::sha256(MARKED.as_bytes())),
    );
    let preview = assess(&plan, &root);
    assert!(
        preview
            .of_kind(l2r_apply::ActionKind::Delete)
            .any(|a| a.path.as_str() == "src/planted.rs"),
        "the preview must show the deletion the record would authorize"
    );
    apply(&plan, &approved_after_preview(&plan, &root), &root).expect("reviewed deletion");
    assert!(!planted.exists());
}

#[test]
fn a_forged_digest_cannot_make_a_hand_edit_replaceable() {
    // The implementer edited a generated file. The attacker (or a careless tool)
    // updates the lock so the edit looks unmodified. The record, made before the
    // edit, names the original content, so the edit survives.
    let (root, _victim) = fixture("forged-digest-replace");
    let plan = plan_for(&edited_signal_model());
    let original = example_plan();
    let approval = approved(&plan, &root); // lists replacements of the ORIGINAL content
    let target = "src/crossing/signal/mod.rs";
    let edited = format!(
        "{}// hand edit\n",
        fs::read_to_string(root.join(target)).unwrap()
    );
    fs::write(root.join(target), &edited).unwrap();
    rewrite_lock_digest(&root, target, &digest::sha256(edited.as_bytes()));
    let why = refusals(apply(&plan, &approval, &root));
    assert_eq!(
        fs::read_to_string(root.join(target)).unwrap(),
        edited,
        "the edit was replaced"
    );
    assert!(
        why.contains(target) && why.contains("not authorized"),
        "{why}"
    );
    assert_ne!(original.digest(), plan.digest());
}

#[test]
fn replacing_a_generated_file_needs_the_record_to_name_it() {
    // A record that authorizes nothing (as one made against an empty tree does)
    // never permits an update, though the lock and the files are all genuine.
    let (root, _victim) = fixture("update-needs-record");
    let next = plan_for(&edited_signal_model());
    let empty = approve(
        &next,
        &next.digest(),
        "reviewer",
        &assess(&next, &root.join("nonexistent")),
    )
    .expect("approves");
    assert!(empty.authorized_changes.is_empty());
    let before = fs::read(root.join("src/crossing/signal/mod.rs")).unwrap();
    let why = refusals(apply(&next, &empty, &root));
    assert!(
        why.contains("replacing") && why.contains("not authorized"),
        "{why}"
    );
    assert_eq!(
        fs::read(root.join("src/crossing/signal/mod.rs")).unwrap(),
        before
    );
    // With the record a preview would have produced, the same update proceeds.
    apply(&next, &approved_after_preview(&next, &root), &root).expect("reviewed update");
}

#[test]
fn an_older_approval_format_is_refused_with_instructions() {
    let (root, _victim) = fixture("v0-approval");
    let plan = plan_for(&example_model());
    let mut record = approved_after_preview(&plan, &root);
    record.format = "l2r.approval/v0".into();
    let err = apply(&plan, &record, &root).unwrap_err();
    assert!(err.to_string().contains("approve it again"), "{err}");
}

fn edited_signal_model() -> Model {
    let mut model = example_model();
    model
        .components
        .iter_mut()
        .find(|c| c.id == "signal")
        .unwrap()
        .responsibility = "Cycles vehicle and pedestrian phases (revised).".into();
    model
}

fn example_plan() -> Plan {
    plan_for(&example_model())
}
