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

fn approved(plan: &Plan) -> Approval {
    approve(plan, &plan.digest(), "test").expect("approves")
}

fn edited_model() -> Model {
    let mut model = example_model();
    let signal = model
        .components
        .iter_mut()
        .find(|c| c.id == "signal")
        .unwrap();
    signal.responsibility.push_str(" (revised)");
    model
}

/// An output tree `<scratch>/out` holding the generated example, and a victim
/// file `<scratch>/victim.txt` outside it.
fn fixture(label: &str) -> (PathBuf, PathBuf) {
    let base = scratch_dir(label);
    let root = base.join("out");
    let plan = plan_for(&example_model());
    apply(&plan, &approved(&plan), &root).expect("initial generation");
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
    let result = apply(&plan, &approved(&plan), &root);
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
    let result = apply(&plan, &approved(&plan), &root);
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

#[cfg(unix)]
#[test]
fn a_planted_temporary_symlink_cannot_redirect_a_write() {
    let (root, victim) = fixture("tmp-symlink");
    let dir = root.join("src/crossing/signal");
    for name in [".mod.rs.l2r-tmp", ".l2r.lock.json.l2r-tmp"] {
        let _ = fs::remove_file(dir.join(name));
    }
    std::os::unix::fs::symlink(&victim, dir.join(".mod.rs.l2r-tmp")).unwrap();
    std::os::unix::fs::symlink(&victim, root.join(".l2r.lock.json.l2r-tmp")).unwrap();

    let next = plan_for(&edited_model());
    let _ = apply(&next, &approved(&next), &root);
    assert_eq!(
        fs::read_to_string(&victim).unwrap(),
        "outside the output tree\n",
        "a write followed a planted symlink out of the tree"
    );
}

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
