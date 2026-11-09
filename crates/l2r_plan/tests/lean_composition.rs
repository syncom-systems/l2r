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

const APPROVAL: &str = include_str!("../../../examples/crossing/architecture.approval.json");
const SEMANTICS: &str = include_str!("../../../lean/L2R/Async.lean");

fn header(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix(&format!("-- {key}: ")))
        .unwrap_or_else(|| panic!("no `-- {key}:` header line"))
        .to_string()
}

fn approval_field(key: &str) -> String {
    let record: serde_json::Value = serde_json::from_str(APPROVAL).expect("approval parses");
    record[key].as_str().expect("string field").to_string()
}

#[test]
fn the_derived_composition_names_exactly_what_it_was_derived_from() {
    // The proofs are about this file. Its header must tie it to the same model,
    // inventory and generator identity that the approval record names, and to
    // the exact semantics file it imports, so the evidence cannot silently be
    // about a different architecture or a different `l2r.async` than approved.
    assert_eq!(header(COMMITTED, "model"), approval_field("model_digest"));
    assert_eq!(
        header(COMMITTED, "inventory"),
        approval_field("inventory_digest")
    );
    assert_eq!(header(COMMITTED, "generator"), approval_field("generator"));
    assert_eq!(
        header(COMMITTED, "semantics"),
        format!(
            "{} {}",
            l2r_plan::SEMANTICS_VERSION,
            l2r_model::digest::sha256(SEMANTICS.as_bytes())
        )
    );
    assert_eq!(
        l2r_plan::semantics_digest(),
        l2r_model::digest::sha256(SEMANTICS.as_bytes())
    );
}

#[test]
fn a_change_to_the_semantics_source_would_change_every_derivation() {
    // The digest line is the only link between the derived text and the
    // semantics file; it must be present and be a digest.
    assert!(l2r_plan::semantics_digest().starts_with("sha256:"));
    assert!(COMMITTED.contains(&format!(
        "-- semantics: l2r.async/v0 {}",
        l2r_plan::semantics_digest()
    )));
}
