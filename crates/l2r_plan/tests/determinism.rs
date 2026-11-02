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

/// Approved generation behavior must never change silently. Each profile's output
/// for the example is pinned here; changing what a profile generates requires a
/// new profile version (e.g. `transition-core/v1`), and v0 keeps producing
/// exactly these bytes. Do not update these digests to make a v0 change pass.
#[test]
fn v0_profile_output_is_frozen() {
    let plan = plan_of(&example_model());
    let mut by_profile: std::collections::BTreeMap<&str, Vec<(&str, &str)>> = Default::default();
    for artifact in &plan.artifacts {
        by_profile
            .entry(artifact.profile.as_str())
            .or_default()
            .push((artifact.path.as_str(), artifact.content.as_str()));
    }
    let digests: Vec<(&str, String)> = by_profile
        .iter()
        .map(|(profile, files)| (*profile, l2r_model::digest::digest_of(files)))
        .collect();
    let expected = [
        (
            "composite/v0",
            "sha256:5acb1f55460425ec22557c81d6d2de483d359e300243c3ec8d5f36d36ff7df0b",
        ),
        (
            "crate-root/v0",
            "sha256:74278f5b2f60a73975cb0c2c4766a9ebf90c00b901ea901465e5d5012c430dc3",
        ),
        (
            "transition-core/v0",
            "sha256:306371c46d0c0c66b3bae53c73d2c634ce9a821815e1a33630fb4a83fa9fbd82",
        ),
    ];
    let expected: Vec<(&str, String)> = expected.iter().map(|(p, d)| (*p, d.to_string())).collect();
    assert_eq!(digests, expected);
}
