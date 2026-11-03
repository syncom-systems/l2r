//! Realistic architecture changes after implementation work has started: the
//! invariants (implementation preserved, ownership explicit, approvals bound to
//! the plan) must hold beyond the original happy path.

use std::fs;
use std::path::{Path, PathBuf};

use l2r_apply::{ActionKind, ApplyError, Approval, ApprovalError, apply, approve, assess, verify};
use l2r_model::{Component, Constructor, Inventory, Model};
use l2r_plan::{GeneratorIdentity, Plan, plan};
use l2r_testing::{example_inventory, example_model, scratch_dir};
use l2r_validate::validate;

fn plan_with(model: &Model, inventory: &Inventory) -> Plan {
    plan(
        &validate(model, inventory).expect("valid"),
        &GeneratorIdentity::current(),
    )
    .expect("plans")
}

fn plan_for(model: &Model) -> Plan {
    plan_with(model, &example_inventory())
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

const WORK: &str = "// the implementer's signal logic\n";

/// A generated example with real implementation work in the signal machine.
fn started(label: &str) -> std::path::PathBuf {
    let root = scratch_dir(label).join("out");
    let first = plan_for(&example_model());
    apply(&first, &approved(&first, &root), &root).expect("generates");
    fs::write(root.join("src/crossing/signal/imp.rs"), WORK).unwrap();
    root
}

fn component<'a>(model: &'a mut Model, id: &str) -> &'a mut Component {
    model.components.iter_mut().find(|c| c.id == id).unwrap()
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap()
}

#[test]
fn renaming_a_module_requires_moving_the_implementation_explicitly() {
    let root = started("rename-module");
    let mut model = example_model();
    component(&mut model, "signal").rust_module = "signal_head".into();
    let next = plan_for(&model);

    let conflicts = assess(&next, &root).conflicts;
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].path, "src/crossing/signal_head/imp.rs");
    assert!(
        conflicts[0]
            .reason
            .contains("moves from `src/crossing/signal/imp.rs`")
    );
    assert!(
        apply(&next, &approved(&next, &root), &root).is_err(),
        "no stub may silently replace the work"
    );

    // The implementer moves the file, as the conflict says; generation then proceeds.
    fs::create_dir_all(root.join("src/crossing/signal_head")).unwrap();
    fs::rename(
        root.join("src/crossing/signal/imp.rs"),
        root.join("src/crossing/signal_head/imp.rs"),
    )
    .unwrap();
    let report = apply(&next, &approved(&next, &root), &root).expect("applies after the move");
    assert_eq!(read(&root, "src/crossing/signal_head/imp.rs"), WORK);
    let deleted: Vec<_> = report
        .assessment
        .of_kind(ActionKind::Delete)
        .map(|a| a.path.to_string())
        .collect();
    assert_eq!(
        deleted,
        [
            "src/crossing/signal/contract_tests.rs",
            "src/crossing/signal/mod.rs"
        ]
    );
}

#[test]
fn moving_machines_under_a_new_parent_moves_their_implementations() {
    let root = started("move-parent");
    let mut model = example_model();
    model.components.push(Component {
        id: "kerb".into(),
        name: "Kerb unit".into(),
        parent: Some("crossing".into()),
        profile: "composite/v0".into(),
        responsibility: "Groups the machines at the kerb.".into(),
        rust_module: "kerb".into(),
        notes: None,
        implementation_budget_lines: None,
        behavior: None,
        mailbox_capacity: None,
    });
    component(&mut model, "signal").parent = Some("kerb".into());
    // Moving only one machine would break the sibling-only message rule.
    assert!(validate(&model, &example_inventory()).is_err());
    component(&mut model, "request_latch").parent = Some("kerb".into());
    let next = plan_for(&model);
    let conflicts = assess(&next, &root).conflicts;
    let moved: Vec<_> = conflicts.iter().map(|c| c.path.as_str()).collect();
    assert!(
        moved.contains(&"src/crossing/kerb/signal/imp.rs"),
        "{conflicts:?}"
    );
    assert_eq!(read(&root, "src/crossing/signal/imp.rs"), WORK);
}

#[test]
fn renaming_a_component_id_keeps_its_files_and_work() {
    let root = started("rename-id");
    let mut model = example_model();
    component(&mut model, "signal").id = "crossing_signal".into();
    for edge in &mut model.messages {
        for end in [&mut edge.from, &mut edge.to] {
            if end == "signal" {
                *end = "crossing_signal".into();
            }
        }
    }
    let next = plan_for(&model);
    apply(&next, &approved(&next, &root), &root).expect("same paths, same ownership");
    assert_eq!(read(&root, "src/crossing/signal/imp.rs"), WORK);
}

#[test]
fn turning_a_machine_into_a_composite_is_rejected_until_it_is_well_formed() {
    let mut model = example_model();
    component(&mut model, "signal").profile = "composite/v0".into();
    let codes: Vec<_> = validate(&model, &example_inventory())
        .unwrap_err()
        .iter()
        .map(|d| d.code)
        .collect();
    assert!(codes.contains(&"L2R-S008"), "{codes:?}");
}

#[test]
fn a_new_lean_constructor_changes_the_contract_but_not_the_implementation() {
    let root = started("new-constructor");
    let mut inventory = example_inventory();
    let state = inventory
        .declarations
        .iter_mut()
        .find(|d| d.name == "Crossing.Signal.State")
        .unwrap();
    state.constructors.push(Constructor {
        name: "Crossing.Signal.State.maintenance".into(),
        arity: 0,
    });
    let next = plan_with(&example_model(), &inventory);
    let first = plan_for(&example_model());
    assert_eq!(
        verify(&approved(&first, Path::new("no-such-tree")), &next),
        Err(ApprovalError::Stale {
            reasons: vec!["the Lean inventory changed".into()]
        })
    );
    let report = apply(&next, &approved(&next, &root), &root).expect("applies");
    let updated: Vec<_> = report
        .assessment
        .of_kind(ActionKind::Update)
        .map(|a| a.path.to_string())
        .collect();
    assert_eq!(updated, ["src/crossing/signal/mod.rs"]);
    assert!(read(&root, "src/crossing/signal/mod.rs").contains("Maintenance,"));
    // The implementation must now handle the new state; L2R never edits it to do so.
    assert_eq!(read(&root, "src/crossing/signal/imp.rs"), WORK);
}

#[test]
fn an_old_approval_never_applies_to_an_evolved_plan() {
    let root = started("old-approval");
    let mut model = example_model();
    component(&mut model, "request_latch").notes = Some("New instruction.".into());
    let next = plan_for(&model);
    let old = approved(&plan_for(&example_model()), Path::new("no-such-tree"));
    assert!(matches!(
        apply(&next, &old, &root),
        Err(ApplyError::Approval(_))
    ));
}

#[test]
fn renaming_id_and_module_together_never_creates_a_silent_stub() {
    let root = started("rename-both");
    let mut model = example_model();
    let signal = component(&mut model, "signal");
    signal.id = "crossing_signal".into();
    signal.rust_module = "crossing_signal".into();
    for edge in &mut model.messages {
        for end in [&mut edge.from, &mut edge.to] {
            if end == "signal" {
                *end = "crossing_signal".into();
            }
        }
    }
    let next = plan_for(&model);
    let conflicts = assess(&next, &root).conflicts;
    let flagged: Vec<_> = conflicts.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(
        flagged,
        ["src/crossing/crossing_signal/imp.rs"],
        "{conflicts:?}"
    );
    assert!(
        conflicts[0].reason.contains("src/crossing/signal/imp.rs"),
        "{conflicts:?}"
    );
    assert!(apply(&next, &approved(&next, &root), &root).is_err());
    assert_eq!(read(&root, "src/crossing/signal/imp.rs"), WORK);
}
