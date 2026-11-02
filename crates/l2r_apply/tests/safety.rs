//! The approval boundary and the write rules hold under hostile and stale conditions.

use std::fs;
use std::path::Path;

use l2r_apply::{
    ActionKind, ApplyError, Approval, ApprovalError, apply, approve, assess, check, lock,
};
use l2r_model::{Inventory, Model};
use l2r_plan::{GeneratorIdentity, Plan, plan};
use l2r_testing::{example_inventory, example_model, scratch_dir};
use l2r_validate::validate;

fn plan_for(model: &Model, inventory: &Inventory) -> Plan {
    plan(
        &validate(model, inventory).expect("valid"),
        &GeneratorIdentity::current(),
    )
    .expect("plans")
}

fn example_plan() -> Plan {
    plan_for(&example_model(), &example_inventory())
}

fn approved(plan: &Plan) -> Approval {
    approve(plan, &plan.digest(), "architect").expect("approves")
}

fn generate(plan: &Plan, root: &Path) {
    apply(plan, &approved(plan), root).expect("applies");
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).expect("file exists")
}

fn edited_model() -> Model {
    let mut model = example_model();
    let signal = model
        .components
        .iter_mut()
        .find(|c| c.id == "signal")
        .unwrap();
    signal.responsibility = "Cycles vehicle and pedestrian phases (revised).".into();
    model
}

const IMPL: &str = "src/crossing/signal/imp.rs";
const CONTRACT: &str = "src/crossing/signal/mod.rs";

#[test]
fn a_fresh_generation_writes_every_planned_file_and_the_lock() {
    let root = scratch_dir("fresh");
    let plan = example_plan();
    generate(&plan, &root);
    for artifact in &plan.artifacts {
        assert_eq!(
            read(&root, &artifact.path),
            artifact.content,
            "{}",
            artifact.path
        );
    }
    let lock = lock::Lock::from_json(&read(&root, lock::LOCK_FILE)).unwrap();
    assert_eq!(lock.plan_digest, plan.digest());
    assert!(check(&example_model(), &plan, &root).is_empty());
}

#[test]
fn regeneration_preserves_implementation_work() {
    let root = scratch_dir("preserve");
    let plan = example_plan();
    generate(&plan, &root);
    fs::write(root.join(IMPL), "// the implementer's work\n").unwrap();

    generate(&plan, &root);
    assert_eq!(read(&root, IMPL), "// the implementer's work\n");

    let next = plan_for(&edited_model(), &example_inventory());
    let report = apply(&next, &approved(&next), &root).expect("applies");
    assert_eq!(read(&root, IMPL), "// the implementer's work\n");
    assert!(read(&root, CONTRACT).contains("(revised)"));
    let updated: Vec<_> = report
        .assessment
        .of_kind(ActionKind::Update)
        .map(|a| a.path.as_str())
        .collect();
    assert_eq!(updated, [CONTRACT]);
}

#[test]
fn an_approval_requires_the_digest_the_architect_saw() {
    let plan = example_plan();
    let other = plan_for(&edited_model(), &example_inventory());
    let err = approve(&plan, &other.digest(), "architect").unwrap_err();
    assert!(matches!(err, ApprovalError::DigestMismatch { .. }));
    assert!(matches!(
        approve(&plan, &plan.digest(), "  "),
        Err(ApprovalError::MissingApprover)
    ));
}

#[test]
fn a_stale_approval_is_refused_and_nothing_is_written() {
    let root = scratch_dir("stale");
    let old_approval = approved(&example_plan());
    let next = plan_for(&edited_model(), &example_inventory());
    match apply(&next, &old_approval, &root) {
        Err(ApplyError::Approval(ApprovalError::Stale { reasons })) => {
            assert_eq!(reasons, ["the architecture model changed"]);
        }
        other => panic!("expected a stale approval, got {other:?}"),
    }
    assert!(!root.join("BUILD.bazel").exists());
}

#[test]
fn a_lean_change_makes_an_approval_stale() {
    let approval = approved(&example_plan());
    let mut inventory = example_inventory();
    inventory.lean_version = "4.99.0".into();
    let next = plan_for(&example_model(), &inventory);
    let err = l2r_apply::verify(&approval, &next).unwrap_err();
    assert_eq!(
        err,
        ApprovalError::Stale {
            reasons: vec!["the Lean inventory changed".into()]
        }
    );
}

#[test]
fn a_generator_change_makes_an_approval_stale() {
    let approval = approved(&example_plan());
    let validated = validate(&example_model(), &example_inventory()).unwrap();
    let next = plan(
        &validated,
        &GeneratorIdentity {
            name: "l2r".into(),
            version: "9.9.9".into(),
        },
    )
    .unwrap();
    let ApprovalError::Stale { reasons } = l2r_apply::verify(&approval, &next).unwrap_err() else {
        panic!("expected stale");
    };
    assert!(reasons[0].contains("generator changed"), "{reasons:?}");
}

#[test]
fn an_edited_generated_file_blocks_the_whole_write() {
    let root = scratch_dir("all-or-nothing");
    generate(&example_plan(), &root);
    // The next plan updates the signal contract; the edit is in a different file.
    fs::write(root.join("src/lib.rs"), "// hand edit\n").unwrap();
    let before = read(&root, CONTRACT);

    let next = plan_for(&edited_model(), &example_inventory());
    match apply(&next, &approved(&next), &root) {
        Err(ApplyError::Conflicts(conflicts)) => {
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].path, "src/lib.rs");
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
    assert_eq!(
        read(&root, CONTRACT),
        before,
        "no file may be written when any conflict exists"
    );
    assert_eq!(read(&root, "src/lib.rs"), "// hand edit\n");
}

#[test]
fn a_foreign_file_in_a_generated_path_is_a_conflict() {
    let root = scratch_dir("foreign");
    fs::write(root.join("BUILD.bazel"), "# someone else's package\n").unwrap();
    let plan = example_plan();
    let err = apply(&plan, &approved(&plan), &root).unwrap_err();
    assert!(
        matches!(err, ApplyError::Conflicts(ref c) if c[0].path == "BUILD.bazel"),
        "{err}"
    );
    assert_eq!(read(&root, "BUILD.bazel"), "# someone else's package\n");
    assert!(!root.join("src").exists());
}

#[cfg(unix)]
#[test]
fn symlinks_inside_the_output_are_refused() {
    let root = scratch_dir("symlink");
    let outside = scratch_dir("outside");
    std::os::unix::fs::symlink(&outside, root.join("src")).unwrap();
    let plan = example_plan();
    let err = apply(&plan, &approved(&plan), &root).unwrap_err();
    assert!(matches!(err, ApplyError::Conflicts(_)), "{err}");
    assert_eq!(
        fs::read_dir(&outside).unwrap().count(),
        0,
        "nothing may be written through the link"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_output_directory_is_refused() {
    let base = scratch_dir("linked-root");
    let real = base.join("real");
    fs::create_dir(&real).unwrap();
    let link = base.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let plan = example_plan();
    assert!(apply(&plan, &approved(&plan), &link).is_err());
    assert_eq!(fs::read_dir(&real).unwrap().count(), 0);
}

#[test]
fn a_malformed_lock_is_a_conflict_not_a_reset() {
    let root = scratch_dir("bad-lock");
    generate(&example_plan(), &root);
    fs::write(root.join(lock::LOCK_FILE), "{ not json").unwrap();
    let plan = example_plan();
    let assessment = assess(&plan, &root);
    assert_eq!(assessment.conflicts[0].path, lock::LOCK_FILE);
    assert!(apply(&plan, &approved(&plan), &root).is_err());
}

#[test]
fn an_ownership_change_requires_manual_migration() {
    let root = scratch_dir("ownership");
    let plan = example_plan();
    generate(&plan, &root);
    let mut recorded = lock::Lock::from_json(&read(&root, lock::LOCK_FILE)).unwrap();
    let entry = recorded.files.iter_mut().find(|e| e.path == IMPL).unwrap();
    entry.ownership = lock::LockOwnership::Generated;
    fs::write(root.join(lock::LOCK_FILE), recorded.to_json()).unwrap();
    let conflicts = assess(&plan, &root).conflicts;
    assert_eq!(conflicts.len(), 1);
    assert!(
        conflicts[0].reason.contains("ownership"),
        "{:?}",
        conflicts[0]
    );
}

#[test]
fn removing_a_component_deletes_its_generated_files_but_keeps_implementation() {
    let root = scratch_dir("removal");
    generate(&example_plan(), &root);
    fs::write(root.join(IMPL), "// signal work\n").unwrap();

    let mut model = example_model();
    model.components.retain(|c| c.id != "signal");
    model.messages.clear();
    let next = plan_for(&model, &example_inventory());
    let report = apply(&next, &approved(&next), &root).expect("applies");

    let deleted: Vec<_> = report
        .assessment
        .of_kind(ActionKind::Delete)
        .map(|a| a.path.as_str())
        .collect();
    assert_eq!(deleted, ["src/crossing/signal/contract_tests.rs", CONTRACT]);
    let orphaned: Vec<_> = report
        .assessment
        .of_kind(ActionKind::Orphan)
        .map(|a| a.path.as_str())
        .collect();
    assert_eq!(orphaned, ["src/crossing/routes.rs", IMPL]);
    assert_eq!(read(&root, IMPL), "// signal work\n");
    assert!(!root.join(CONTRACT).exists());
}

#[test]
fn check_reports_drift_and_budget_overruns() {
    let root = scratch_dir("check");
    let plan = example_plan();
    generate(&plan, &root);
    fs::write(root.join(CONTRACT), "// drifted\n").unwrap();
    fs::write(root.join(IMPL), "//\n".repeat(61)).unwrap();
    let findings = check(&example_model(), &plan, &root);
    let paths: Vec<_> = findings.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, [IMPL, CONTRACT], "{findings:?}");
    assert!(findings[0].message.contains("budget of 60"));
}

#[test]
fn check_reports_a_plan_that_was_never_applied() {
    let root = scratch_dir("unapplied");
    generate(&example_plan(), &root);
    let next = plan_for(&edited_model(), &example_inventory());
    let findings = check(&edited_model(), &next, &root);
    assert!(
        findings
            .iter()
            .any(|f| f.path == CONTRACT && f.message.contains("out of date"))
    );
    assert!(findings.iter().any(|f| f.path == lock::LOCK_FILE));
}
