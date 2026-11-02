//! Planning is deterministic and every input that shapes the output is bound
//! into the plan digest.

use l2r_model::Model;
use l2r_plan::{GeneratorIdentity, Ownership, Plan, plan};
use l2r_testing::{example_inventory, example_model};
use l2r_validate::validate;

fn plan_of(model: &Model) -> Plan {
    let validated = validate(model, &example_inventory()).expect("valid");
    plan(&validated, &GeneratorIdentity::current()).expect("plans")
}

#[test]
fn identical_input_gives_identical_plans() {
    let a = plan_of(&example_model());
    let b = plan_of(&example_model());
    assert_eq!(a, b);
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn declaration_order_does_not_change_the_plan() {
    let mut shuffled = example_model();
    shuffled.components.reverse();
    shuffled.messages.reverse();
    for c in &mut shuffled.components {
        if let Some(b) = &mut c.behavior {
            b.invariants.reverse();
        }
    }
    assert_eq!(
        plan_of(&example_model()).digest(),
        plan_of(&shuffled).digest()
    );
}

#[test]
fn artifacts_are_sorted_unique_and_relative() {
    let plan = plan_of(&example_model());
    let paths: Vec<&str> = plan.artifacts.iter().map(|a| a.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(paths, sorted);
    assert!(
        paths
            .iter()
            .all(|p| !p.starts_with('/') && !p.contains(".."))
    );
}

#[test]
fn the_example_plans_the_expected_files() {
    let plan = plan_of(&example_model());
    let listing: Vec<(&str, Ownership)> = plan
        .artifacts
        .iter()
        .map(|a| (a.path.as_str(), a.ownership))
        .collect();
    use Ownership::*;
    assert_eq!(
        listing,
        [
            ("BUILD.bazel", Generated),
            ("src/crossing/mod.rs", Generated),
            ("src/crossing/request_latch/contract_tests.rs", Generated),
            ("src/crossing/request_latch/imp.rs", Implementation),
            ("src/crossing/request_latch/mod.rs", Generated),
            ("src/crossing/routes.rs", Implementation),
            ("src/crossing/signal/contract_tests.rs", Generated),
            ("src/crossing/signal/imp.rs", Implementation),
            ("src/crossing/signal/mod.rs", Generated),
            ("src/lib.rs", Generated),
        ]
    );
}

#[test]
fn every_artifact_traces_to_a_component() {
    let plan = plan_of(&example_model());
    for artifact in &plan.artifacts {
        assert!(!artifact.components.is_empty(), "{}", artifact.path);
    }
    let signal = plan.artifact("src/crossing/signal/mod.rs").unwrap();
    assert!(signal.lean.contains(&"Crossing.Signal.step".to_string()));
}

#[test]
fn generated_content_has_no_ambient_data() {
    let plan = plan_of(&example_model());
    for artifact in &plan.artifacts {
        let c = &artifact.content;
        assert!(
            !c.contains("/Users/") && !c.contains("/home/") && !c.contains("/tmp/"),
            "{}",
            artifact.path
        );
        assert!(!c.contains('\r'), "{}", artifact.path);
    }
}

#[test]
fn a_model_edit_changes_the_digest() {
    let mut edited = example_model();
    edited.components[1].responsibility.push_str(" Revised.");
    assert_ne!(
        plan_of(&example_model()).digest(),
        plan_of(&edited).digest()
    );
}

#[test]
fn an_inventory_change_changes_the_digest() {
    let validated = validate(&example_model(), &example_inventory()).unwrap();
    let base = plan(&validated, &GeneratorIdentity::current()).unwrap();
    let mut inventory = example_inventory();
    inventory.lean_version = "4.99.0".into();
    let validated = validate(&example_model(), &inventory).unwrap();
    let changed = plan(&validated, &GeneratorIdentity::current()).unwrap();
    // No file content changes, yet the approval must not carry over.
    assert_eq!(base.artifacts, changed.artifacts);
    assert_ne!(base.digest(), changed.digest());
}

#[test]
fn a_generator_version_change_changes_the_digest() {
    let validated = validate(&example_model(), &example_inventory()).unwrap();
    let current = plan(&validated, &GeneratorIdentity::current()).unwrap();
    let next = GeneratorIdentity {
        name: "l2r".into(),
        version: "0.2.0".into(),
    };
    assert_ne!(current.digest(), plan(&validated, &next).unwrap().digest());
}

#[test]
fn a_profile_version_change_is_rejected_until_implemented() {
    // A profile bump is not silently mapped onto the old profile.
    let mut model = example_model();
    model
        .components
        .iter_mut()
        .find(|c| c.id == "signal")
        .unwrap()
        .profile = "transition-core/v1".into();
    assert!(validate(&model, &example_inventory()).is_err());
}
