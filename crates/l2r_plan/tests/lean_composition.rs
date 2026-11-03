//! The committed derived Lean composition must equal a fresh derivation from the
//! committed model and inventory, so the Lean proofs about the composition are
//! about the structure that is actually approved.

use l2r_plan::{lean_composition, lean_composition_path};
use l2r_testing::{example_inventory, example_model};
use l2r_validate::validate;

const COMMITTED: &str =
    include_str!("../../../examples/crossing/composition/L2RDerived/crossing.lean");

#[test]
fn the_committed_derived_composition_is_current() {
    let validated = validate(&example_model(), &example_inventory()).expect("valid");
    assert_eq!(
        lean_composition_path(validated.model()),
        "L2RDerived/crossing.lean"
    );
    let fresh = lean_composition(&validated).expect("derives");
    assert!(
        fresh == COMMITTED,
        "examples/crossing/composition/L2RDerived/crossing.lean is stale; regenerate it with \
         `l2r lean-composition examples/crossing/architecture.toml`\n--- fresh ---\n{fresh}"
    );
}

#[test]
fn derivation_is_deterministic_and_bound_to_its_inputs() {
    let validated = validate(&example_model(), &example_inventory()).expect("valid");
    let once = lean_composition(&validated).unwrap();
    assert_eq!(once, lean_composition(&validated).unwrap());
    let mut model = example_model();
    model.components[0].responsibility.push_str(" (revised)");
    let changed = lean_composition(&validate(&model, &example_inventory()).unwrap()).unwrap();
    assert_ne!(
        once, changed,
        "the header must record the model it was derived from"
    );
}

#[test]
fn edges_follow_the_approved_model() {
    // Removing an edge removes its route; the derivation never invents one.
    let mut model = example_model();
    model.messages.retain(|m| m.id != "signal_to_latch");
    let text = lean_composition(&validate(&model, &example_inventory()).unwrap()).unwrap();
    assert!(!text.contains("Crossing.Routes.signalToLatch o"), "{text}");
    assert!(text.contains("Crossing.Routes.latchToSignal o"), "{text}");
}
