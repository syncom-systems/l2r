//! Interrupted generations: every interruption point is recoverable to exactly
//! the approved plan, implementation files are never altered, and nothing
//! outside the output tree changes. Built against `:l2r_apply_fault`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use l2r_apply::fault::{self, Mode};
use l2r_apply::journal::JOURNAL_FILE;
use l2r_apply::lock::{LOCK_FILE, Lock};
use l2r_apply::{ApplyError, Approval, apply, approve, assess, check, pending, recover};
use l2r_model::Model;
use l2r_plan::{GeneratorIdentity, Ownership, Plan, plan};
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

/// A change that exercises every step kind: two replacements, one creation and
/// two deletions (the signal machine is removed).
fn next_model() -> Model {
    let mut model = example_model();
    model.components.retain(|c| c.id != "signal");
    model.messages.clear();
    let latch = model
        .components
        .iter_mut()
        .find(|c| c.id == "request_latch")
        .unwrap();
    latch.responsibility.push_str(" (revised)");
    model
}

const IMPLEMENTATIONS: &[&str] = &[
    "src/crossing/request_latch/imp.rs",
    "src/crossing/routes.rs",
    "src/crossing/signal/imp.rs",
];

/// Every regular file under `dir`, relative path → content.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let rel = path.strip_prefix(base).unwrap().display().to_string();
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The scratch base holds `out/` (the tree) and `victim.txt` (outside it).
struct Fixture {
    base: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let base = scratch_dir(label);
        let root = base.join("out");
        let first = plan_for(&example_model());
        apply(&first, &approved(&first, &root), &root).expect("initial generation");
        for path in IMPLEMENTATIONS {
            fs::write(
                root.join(path),
                format!("// implementer's work in {path}\n"),
            )
            .unwrap();
        }
        // A generated file removed by hand becomes a Create step.
        fs::remove_file(root.join("src/crossing/request_latch/contract_tests.rs")).unwrap();
        fs::write(base.join("victim.txt"), "outside\n").unwrap();
        Fixture { base, root }
    }

    fn outside(&self) -> BTreeMap<String, Vec<u8>> {
        snapshot(&self.base)
            .into_iter()
            .filter(|(p, _)| !p.starts_with("out/"))
            .collect()
    }
}

fn assert_matches_plan(root: &Path, plan: &Plan) {
    let files = snapshot(root);
    for artifact in &plan.artifacts {
        let found = files
            .get(artifact.path.as_str())
            .unwrap_or_else(|| panic!("{} missing", artifact.path));
        if artifact.ownership == Ownership::Generated {
            assert_eq!(found, artifact.content.as_bytes(), "{}", artifact.path);
        }
    }
    let lock = Lock::from_json(&String::from_utf8(files[LOCK_FILE].clone()).unwrap()).unwrap();
    assert_eq!(lock, Lock::for_plan(plan));
    assert!(!files.contains_key(JOURNAL_FILE));
    assert!(
        files.keys().all(|p| !p.contains(".l2r-tmp")),
        "temporary files remain: {:?}",
        files.keys()
    );
}

fn assert_implementations_preserved(root: &Path) {
    for path in IMPLEMENTATIONS {
        let content = fs::read_to_string(root.join(path)).unwrap();
        assert_eq!(
            content,
            format!("// implementer's work in {path}\n"),
            "{path} was altered"
        );
    }
}

fn step_count(fixture: &Fixture, next: &Plan) -> usize {
    let assessment = l2r_apply::assess(next, &fixture.root);
    use l2r_apply::ActionKind::*;
    assessment
        .actions
        .iter()
        .filter(|a| matches!(a.kind, Create | Update | Delete))
        .count()
}

#[test]
fn every_interruption_point_recovers_to_the_approved_plan() {
    let next = plan_for(&next_model());
    let steps = step_count(&Fixture::new("count"), &next);
    assert_eq!(
        steps, 5,
        "the scenario must exercise create, replace and delete"
    );
    // k = 0: journal written, nothing committed; k = steps + 1: lock written, journal not yet removed.
    for k in 0..=steps + 1 {
        let fixture = Fixture::new(&format!("interrupt-{k}"));
        let outside = fixture.outside();
        fault::arm(k, Mode::Interrupt);
        let result = apply(&next, &approved(&next, &fixture.root), &fixture.root);
        fault::disarm();
        assert!(
            matches!(result, Err(ApplyError::Interrupted(_))),
            "k={k}: {result:?}"
        );
        assert!(pending(&fixture.root), "k={k}: journal must remain");
        assert_implementations_preserved(&fixture.root);

        // A new generation is refused while the journal is pending.
        assert!(matches!(
            apply(&next, &approved(&next, &fixture.root), &fixture.root),
            Err(ApplyError::Conflicts(_))
        ));

        recover(&next, &approved(&next, &fixture.root), &fixture.root)
            .unwrap_or_else(|e| panic!("k={k}: {e}"));
        assert_matches_plan(&fixture.root, &next);
        assert_implementations_preserved(&fixture.root);
        assert_eq!(
            fixture.outside(),
            outside,
            "k={k}: something outside the tree changed"
        );
        let findings = check(&next_model(), &next, &fixture.root);
        let only_orphans = findings
            .iter()
            .all(|f| f.message.contains("no longer planned by the model"));
        assert!(only_orphans, "k={k}: {findings:?}");
    }
}

#[test]
fn a_staging_failure_leaves_every_file_unchanged() {
    let next = plan_for(&next_model());
    for k in 1..=3 {
        let fixture = Fixture::new(&format!("stage-{k}"));
        let before = snapshot(&fixture.root);
        fault::arm(k, Mode::FailStaging);
        let result = apply(&next, &approved(&next, &fixture.root), &fixture.root);
        fault::disarm();
        assert!(matches!(result, Err(ApplyError::Staging(_))), "{result:?}");
        assert_eq!(
            snapshot(&fixture.root),
            before,
            "k={k}: staging failure changed the tree"
        );
        assert!(!pending(&fixture.root));
    }
}

#[test]
fn recovery_never_overwrites_a_file_changed_after_interruption() {
    let fixture = Fixture::new("concurrent-edit");
    let next = plan_for(&next_model());
    fault::arm(0, Mode::Interrupt);
    let _ = apply(&next, &approved(&next, &fixture.root), &fixture.root);
    fault::disarm();
    let edited = "src/crossing/request_latch/mod.rs";
    fs::write(
        fixture.root.join(edited),
        "// edited by hand during the interruption\n",
    )
    .unwrap();
    let err = recover(&next, &approved(&next, &fixture.root), &fixture.root).unwrap_err();
    assert!(err.to_string().contains("changed by someone else"), "{err}");
    assert_eq!(
        fs::read_to_string(fixture.root.join(edited)).unwrap(),
        "// edited by hand during the interruption\n"
    );
    assert!(
        pending(&fixture.root),
        "the journal stays until the conflict is resolved"
    );
}

#[test]
fn an_implementation_file_created_during_interruption_wins_over_the_stub() {
    let base = scratch_dir("impl-race");
    let root = base.join("out");
    let first = plan_for(&example_model());
    fault::arm(0, Mode::Interrupt);
    let _ = apply(&first, &approved(&first, &root), &root);
    fault::disarm();
    let imp = "src/crossing/signal/imp.rs";
    fs::create_dir_all(root.join("src/crossing/signal")).unwrap();
    fs::write(root.join(imp), "// written by the implementer first\n").unwrap();
    recover(&first, &approved(&first, &root), &root).expect("recovers");
    assert_eq!(
        fs::read_to_string(root.join(imp)).unwrap(),
        "// written by the implementer first\n"
    );
}

#[test]
fn lost_staged_content_is_restaged_from_the_plan() {
    let fixture = Fixture::new("lost-temp");
    let next = plan_for(&next_model());
    fault::arm(0, Mode::Interrupt);
    let _ = apply(&next, &approved(&next, &fixture.root), &fixture.root);
    fault::disarm();
    for (path, _) in snapshot(&fixture.root)
        .iter()
        .filter(|(p, _)| p.contains(".l2r-tmp"))
    {
        fs::write(fixture.root.join(path), "garbage").unwrap();
    }
    recover(&next, &approved(&next, &fixture.root), &fixture.root).expect("recovers");
    assert_matches_plan(&fixture.root, &next);
}

fn interrupted_journal(label: &str) -> (Fixture, Plan, serde_json::Value) {
    let fixture = Fixture::new(label);
    let next = plan_for(&next_model());
    fault::arm(1, Mode::Interrupt);
    let _ = apply(&next, &approved(&next, &fixture.root), &fixture.root);
    fault::disarm();
    let text = fs::read_to_string(fixture.root.join(JOURNAL_FILE)).unwrap();
    (fixture, next, serde_json::from_str(&text).unwrap())
}

fn write_journal(fixture: &Fixture, journal: &serde_json::Value) {
    fs::write(
        fixture.root.join(JOURNAL_FILE),
        serde_json::to_string(journal).unwrap(),
    )
    .unwrap();
}

#[test]
fn a_tampered_journal_cannot_escape_the_tree() {
    let (fixture, next, mut journal) = interrupted_journal("journal-escape");
    let outside = fixture.outside();
    journal["steps"][4]["path"] = "../victim.txt".into();
    write_journal(&fixture, &journal);
    assert!(matches!(
        recover(&next, &approved(&next, &fixture.root), &fixture.root),
        Err(ApplyError::Journal(_))
    ));
    assert_eq!(fixture.outside(), outside);
}

#[test]
fn a_tampered_journal_cannot_replace_an_implementation_file() {
    let (fixture, next, mut journal) = interrupted_journal("journal-impl");
    let target = "src/crossing/request_latch/imp.rs";
    let steps = journal["steps"].as_array_mut().unwrap();
    let mut forged = steps[0].clone();
    forged["path"] = target.into();
    forged["ownership"] = "implementation".into();
    forged["kind"] = "replace".into();
    forged["before"] =
        l2r_model::digest::sha256(&fs::read(fixture.root.join(target)).unwrap()).into();
    steps.push(forged);
    write_journal(&fixture, &journal);
    assert!(matches!(
        recover(&next, &approved(&next, &fixture.root), &fixture.root),
        Err(ApplyError::Journal(_))
    ));
    assert_implementations_preserved(&fixture.root);
}

#[test]
fn a_journal_for_another_plan_is_refused() {
    let (fixture, _next, _journal) = interrupted_journal("journal-other");
    let other = plan_for(&example_model());
    let err = recover(&other, &approved(&other, &fixture.root), &fixture.root).unwrap_err();
    assert!(err.to_string().contains("the journal is for plan"), "{err}");
}

#[test]
fn a_journal_with_content_outside_the_plan_is_refused() {
    let (fixture, next, mut journal) = interrupted_journal("journal-content");
    journal["steps"][0]["after"] = l2r_model::digest::sha256(b"injected").into();
    write_journal(&fixture, &journal);
    let err = recover(&next, &approved(&next, &fixture.root), &fixture.root).unwrap_err();
    assert!(err.to_string().contains("does not match the plan"), "{err}");
}

#[test]
fn recovery_requires_a_current_approval() {
    let (fixture, next, _journal) = interrupted_journal("journal-approval");
    let other = plan_for(&example_model());
    assert!(matches!(
        recover(&next, &approved(&other, &fixture.root), &fixture.root),
        Err(ApplyError::Approval(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_directory_swapped_for_a_symlink_mid_generation_is_never_followed() {
    let fixture = Fixture::new("swap-dir");
    let next = plan_for(&next_model());
    fault::arm(0, Mode::Interrupt);
    let _ = apply(&next, &approved(&next, &fixture.root), &fixture.root);
    fault::disarm();
    // Replace a directory that still has pending steps with a link to a directory outside the tree.
    let decoy = fixture.base.join("decoy");
    fs::create_dir(&decoy).unwrap();
    let target = fixture.root.join("src/crossing/request_latch");
    fs::rename(&target, fixture.base.join("moved-away")).unwrap();
    std::os::unix::fs::symlink(&decoy, &target).unwrap();

    let err = recover(&next, &approved(&next, &fixture.root), &fixture.root).unwrap_err();
    assert!(err.to_string().contains("symbolic link"), "{err}");
    assert_eq!(
        fs::read_dir(&decoy).unwrap().count(),
        0,
        "a write went through the swapped-in link"
    );
}

#[cfg(unix)]
#[test]
fn symlinks_planted_at_the_exact_temporary_names_are_refused() {
    let fixture = Fixture::new("planted-temps");
    let next = plan_for(&next_model());
    let victim = fixture.base.join("victim.txt");
    let before = snapshot(&fixture.root);
    // Plant a link to the victim at every name this run will use, in every directory.
    fault::fix_run_id("feed");
    let mut dirs = vec![fixture.root.clone()];
    let mut i = 0;
    while i < dirs.len() {
        for entry in fs::read_dir(&dirs[i]).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
        i += 1;
    }
    for dir in &dirs {
        for n in 0..32 {
            std::os::unix::fs::symlink(&victim, dir.join(format!(".l2r-tmp.feed.{n}"))).unwrap();
        }
    }
    let result = apply(&next, &approved(&next, &fixture.root), &fixture.root);
    assert!(matches!(result, Err(ApplyError::Staging(_))), "{result:?}");
    assert_eq!(
        fs::read_to_string(&victim).unwrap(),
        "outside\n",
        "a write followed a planted link"
    );
    let after: BTreeMap<_, _> = snapshot(&fixture.root);
    let unchanged = before.iter().all(|(p, c)| after.get(p) == Some(c));
    assert!(
        unchanged,
        "a staging refusal must leave the tree's files unchanged"
    );
    assert!(!pending(&fixture.root));
}

// ---------------------------------------------------------------------------
// Journal authority. A journal is untrusted: recovery may do only what a
// generation of the same approved plan would do from the recorded prior state.
// ---------------------------------------------------------------------------

fn digest_of_file(fixture: &Fixture, path: &str) -> String {
    l2r_model::digest::sha256(&fs::read(fixture.root.join(path)).unwrap())
}

fn planned_digest(plan: &Plan, path: &str) -> String {
    l2r_model::digest::sha256(plan.artifact(path).unwrap().content.as_bytes())
}

/// The lock must never claim the plan unless the tree is in the plan's state.
fn assert_lock_is_truthful(root: &Path, plan: &Plan) {
    let files = snapshot(root);
    if let Some(bytes) = files.get(LOCK_FILE) {
        let lock = Lock::from_json(&String::from_utf8(bytes.clone()).unwrap()).unwrap();
        if lock == Lock::for_plan(plan) {
            assert_matches_plan(root, plan);
        }
    }
}

#[test]
fn a_journal_cannot_forge_generated_ownership_of_an_implementation_file() {
    let (fixture, next, mut journal) = interrupted_journal("forge-generated");
    let target = "src/crossing/request_latch/imp.rs"; // planned, implementation-owned
    let forged = serde_json::json!({
        "path": target,
        "ownership": "generated",
        "kind": "replace",
        "before": digest_of_file(&fixture, target),
        "after": planned_digest(&next, target),
        "temp": ".l2r-tmp.ab.1",
    });
    journal["steps"].as_array_mut().unwrap().push(forged);
    write_journal(&fixture, &journal);
    let result = recover(&next, &approved(&next, &fixture.root), &fixture.root);
    assert_implementations_preserved(&fixture.root);
    assert!(matches!(result, Err(ApplyError::Journal(_))), "{result:?}");
}

#[test]
fn a_journal_cannot_overwrite_an_edited_generated_file() {
    let (fixture, next, mut journal) = interrupted_journal("forge-edited");
    // Planned, generated, replaced by this plan; edited by hand during the interruption.
    let target = "src/crossing/request_latch/mod.rs";
    let edited = "// hand edit that the lock does not know\n";
    fs::write(fixture.root.join(target), edited).unwrap();
    let steps = journal["steps"].as_array_mut().unwrap();
    let step = steps.iter_mut().find(|s| s["path"] == target).unwrap();
    step["before"] = l2r_model::digest::sha256(edited.as_bytes()).into();
    write_journal(&fixture, &journal);
    let result = recover(&next, &approved(&next, &fixture.root), &fixture.root);
    assert!(result.is_err(), "{result:?}");
    assert_eq!(
        fs::read_to_string(fixture.root.join(target)).unwrap(),
        edited
    );
}

/// Appends a forged delete step for `target` and runs recovery.
fn forged_delete(label: &str, target: &str, plant: Option<&str>) {
    let (fixture, next, mut journal) = interrupted_journal(label);
    if let Some(content) = plant {
        fs::write(fixture.root.join(target), content).unwrap();
    }
    let before = fs::read(fixture.root.join(target)).unwrap();
    journal["steps"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "path": target,
            "ownership": "generated",
            "kind": "delete",
            "before": digest_of_file(&fixture, target),
        }));
    write_journal(&fixture, &journal);
    let result = recover(&next, &approved(&next, &fixture.root), &fixture.root);
    assert_eq!(
        fs::read(fixture.root.join(target)).ok(),
        Some(before),
        "{target} was deleted"
    );
    assert!(matches!(result, Err(ApplyError::Journal(_))), "{result:?}");
}

#[test]
fn a_journal_cannot_delete_a_file_l2r_never_recorded() {
    forged_delete(
        "forge-delete-foreign",
        "src/notes.txt",
        Some("// someone's notes\n"),
    );
}

#[test]
fn a_journal_cannot_delete_an_orphaned_implementation_file() {
    forged_delete("forge-delete-orphan", "src/crossing/signal/imp.rs", None);
}

/// Interrupts after `k` steps, strips every step from the journal, recovers.
fn omitted_steps(k: usize) {
    let fixture = Fixture::new(&format!("omit-{k}"));
    let next = plan_for(&next_model());
    fault::arm(k, Mode::Interrupt);
    let _ = apply(&next, &approved(&next, &fixture.root), &fixture.root);
    fault::disarm();
    let text = fs::read_to_string(fixture.root.join(JOURNAL_FILE)).unwrap();
    let mut journal: serde_json::Value = serde_json::from_str(&text).unwrap();
    journal["steps"] = serde_json::json!([]);
    write_journal(&fixture, &journal);
    let result = recover(&next, &approved(&next, &fixture.root), &fixture.root);
    // Recovery either completes the whole plan or refuses; it never records
    // the plan as applied while it is not.
    assert_lock_is_truthful(&fixture.root, &next);
    assert!(result.is_err(), "an incomplete journal was accepted");
    assert!(pending(&fixture.root), "the journal stays until resolved");
    assert_implementations_preserved(&fixture.root);
}

#[test]
fn a_journal_with_every_step_omitted_cannot_produce_a_false_lock() {
    omitted_steps(0);
}

#[test]
fn a_journal_with_the_remaining_steps_omitted_cannot_produce_a_false_lock() {
    omitted_steps(1);
}

#[test]
fn recovery_refuses_a_prior_lock_swapped_after_the_interruption() {
    let (fixture, next, _journal) = interrupted_journal("swap-lock");
    // Rewrite the prior lock so that the orphaned implementation file looks
    // generated; a generation from this lock would delete it.
    let orphan = "src/crossing/signal/imp.rs";
    let text = fs::read_to_string(fixture.root.join(LOCK_FILE)).unwrap();
    let mut lock: serde_json::Value = serde_json::from_str(&text).unwrap();
    for entry in lock["files"].as_array_mut().unwrap() {
        if entry["path"] == orphan {
            entry["ownership"] = "generated".into();
            entry["digest"] = digest_of_file(&fixture, orphan).into();
        }
    }
    fs::write(
        fixture.root.join(LOCK_FILE),
        serde_json::to_string_pretty(&lock).unwrap(),
    )
    .unwrap();
    let result = recover(&next, &approved(&next, &fixture.root), &fixture.root);
    assert!(matches!(result, Err(ApplyError::Journal(_))), "{result:?}");
    assert_implementations_preserved(&fixture.root);
}

#[test]
fn a_consistently_forged_lock_and_journal_still_cannot_delete_an_unmarked_file() {
    // Forge both: the prior lock records `src/notes.txt` as generated, and the
    // journal binds that forged lock and deletes the file. Recovery then has
    // exactly generation's authority from that lock, which never deletes a file
    // lacking L2R's generated-file marker.
    let (fixture, next, mut journal) = interrupted_journal("forge-both");
    let target = "src/notes.txt";
    fs::write(fixture.root.join(target), "// someone's notes\n").unwrap();
    let text = fs::read_to_string(fixture.root.join(LOCK_FILE)).unwrap();
    let mut lock: serde_json::Value = serde_json::from_str(&text).unwrap();
    lock["files"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "path": target, "ownership": "generated", "component": "signal",
            "digest": digest_of_file(&fixture, target),
        }));
    let forged_lock = serde_json::to_string_pretty(&lock).unwrap();
    fs::write(fixture.root.join(LOCK_FILE), &forged_lock).unwrap();
    journal["prior_lock"] = l2r_model::digest::sha256(forged_lock.as_bytes()).into();
    journal["steps"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "path": target, "ownership": "generated", "kind": "delete",
            "before": digest_of_file(&fixture, target),
        }));
    write_journal(&fixture, &journal);
    let result = recover(&next, &approved(&next, &fixture.root), &fixture.root);
    assert!(
        fixture.root.join(target).exists(),
        "the unmarked file was deleted"
    );
    assert!(result.is_err(), "{result:?}");
    assert_lock_is_truthful(&fixture.root, &next);
}

#[test]
fn a_consistently_forged_lock_marker_and_journal_cannot_delete_without_the_record() {
    // The strongest forgery available inside the output tree: a file that carries
    // L2R's generated-file marker, a lock that records it as generated with its
    // digest, and a journal bound to that lock whose delete step is therefore
    // consistent with everything on disk. Only the approval record, which the
    // tree cannot edit and which never named this file, stands in the way.
    let (fixture, next, mut journal) = interrupted_journal("forge-marked");
    let target = "src/planted.rs";
    fs::write(
        fixture.root.join(target),
        "// @generated by L2R (forged). planted by an attacker\n",
    )
    .unwrap();
    let text = fs::read_to_string(fixture.root.join(LOCK_FILE)).unwrap();
    let mut lock: serde_json::Value = serde_json::from_str(&text).unwrap();
    lock["files"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "path": target, "ownership": "generated", "component": "signal",
            "digest": digest_of_file(&fixture, target),
        }));
    let forged_lock = serde_json::to_string_pretty(&lock).unwrap();
    fs::write(fixture.root.join(LOCK_FILE), &forged_lock).unwrap();
    journal["prior_lock"] = l2r_model::digest::sha256(forged_lock.as_bytes()).into();
    journal["steps"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "path": target, "ownership": "generated", "kind": "delete",
            "before": digest_of_file(&fixture, target),
        }));
    write_journal(&fixture, &journal);
    // The genuine record, made from the tree before the forgery.
    let result = recover(&next, &approved(&next, &fixture.root), &fixture.root);
    assert!(
        fixture.root.join(target).exists(),
        "a forged lock and journal deleted a marked file"
    );
    match result {
        Err(ApplyError::Journal(why)) => assert!(why.contains("approval record"), "{why}"),
        other => panic!("expected a refusal naming the approval record: {other:?}"),
    }
    assert_lock_is_truthful(&fixture.root, &next);
}

#[test]
fn once_the_plans_lock_is_written_recovery_runs_no_step() {
    // Interrupted after the lock was written: the prior lock is gone, so no
    // step can be authorized any more. A forged delete must not run.
    let fixture = Fixture::new("verify-only");
    let next = plan_for(&next_model());
    let steps = step_count(&fixture, &next);
    fault::arm(steps + 1, Mode::Interrupt);
    let _ = apply(&next, &approved(&next, &fixture.root), &fixture.root);
    fault::disarm();
    let target = "src/notes.txt";
    fs::write(
        fixture.root.join(target),
        "// @generated by L2R (x). planted\n",
    )
    .unwrap();
    let text = fs::read_to_string(fixture.root.join(JOURNAL_FILE)).unwrap();
    let mut journal: serde_json::Value = serde_json::from_str(&text).unwrap();
    journal["steps"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "path": target, "ownership": "generated", "kind": "delete",
            "before": digest_of_file(&fixture, target),
        }));
    write_journal(&fixture, &journal);
    recover(&next, &approved(&next, &fixture.root), &fixture.root).expect("completes");
    assert!(
        fixture.root.join(target).exists(),
        "a step ran after the lock was written"
    );
    assert!(!pending(&fixture.root));
}

#[test]
fn a_journal_from_before_prior_lock_binding_is_refused() {
    let (fixture, next, mut journal) = interrupted_journal("journal-v0");
    journal["format"] = "l2r.journal/v0".into();
    journal.as_object_mut().unwrap().remove("prior_lock");
    write_journal(&fixture, &journal);
    let err = recover(&next, &approved(&next, &fixture.root), &fixture.root).unwrap_err();
    assert!(
        err.to_string().contains("unsupported journal format"),
        "{err}"
    );
}

#[test]
fn a_journal_cannot_change_the_ownership_the_plan_gives_a_new_file() {
    // First generation: no prior lock, so only the plan can vouch for ownership.
    let base = scratch_dir("forge-new-ownership");
    let root = base.join("out");
    let first = plan_for(&example_model());
    fault::arm(0, Mode::Interrupt);
    let _ = apply(&first, &approved(&first, &root), &root);
    fault::disarm();
    let text = fs::read_to_string(root.join(JOURNAL_FILE)).unwrap();
    let mut journal: serde_json::Value = serde_json::from_str(&text).unwrap();
    let step = journal["steps"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["path"] == "src/crossing/signal/imp.rs")
        .unwrap();
    step["ownership"] = "generated".into();
    fs::write(root.join(JOURNAL_FILE), journal.to_string()).unwrap();
    let err = recover(&first, &approved(&first, &root), &root).unwrap_err();
    assert!(
        err.to_string().contains("ownership the plan does not give"),
        "{err}"
    );
}
