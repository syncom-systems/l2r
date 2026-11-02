//! Each validation rule rejects a minimal violation of the example model.

use l2r_model::{Behavior, Component, DeclKind, Inventory, MessageEdge, Model};
use l2r_testing::{example_inventory, example_model};
use l2r_validate::{Diagnostic, validate};

fn codes(model: &Model, inventory: &Inventory) -> Vec<&'static str> {
    match validate(model, inventory) {
        Ok(_) => vec![],
        Err(diagnostics) => diagnostics.iter().map(|d: &Diagnostic| d.code).collect(),
    }
}

fn rejects(model: &Model, code: &str) {
    let found = codes(model, &example_inventory());
    assert!(found.contains(&code), "expected {code}, got {found:?}");
}

fn component<'a>(model: &'a mut Model, id: &str) -> &'a mut Component {
    model
        .components
        .iter_mut()
        .find(|c| c.id == id)
        .expect("component exists")
}

fn behavior<'a>(model: &'a mut Model, id: &str) -> &'a mut Behavior {
    component(model, id).behavior.as_mut().expect("machine")
}

#[test]
fn the_example_model_is_valid() {
    let validated = validate(&example_model(), &example_inventory()).expect("valid");
    let signal = validated.machine("signal").expect("signal resolved");
    let names: Vec<_> = signal
        .state
        .variants
        .iter()
        .map(|v| v.rust_name.as_str())
        .collect();
    assert_eq!(
        names,
        ["VehiclesGreen", "VehiclesAmber", "Walk", "Clearance"]
    );
}

#[test]
fn unsupported_schema_is_rejected() {
    let mut model = example_model();
    model.schema = "l2r.architecture/v1".into();
    rejects(&model, "L2R-S001");
}

#[test]
fn invalid_identifiers_are_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").rust_module = "Signal".into();
    rejects(&model, "L2R-S002");
}

#[test]
fn duplicate_component_ids_are_rejected() {
    let mut model = example_model();
    let mut copy = component(&mut model, "signal").clone();
    copy.rust_module = "signal_two".into();
    model.components.push(copy);
    rejects(&model, "L2R-S003");
}

#[test]
fn dangling_parents_are_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").parent = Some("junction".into());
    rejects(&model, "L2R-S004");
}

#[test]
fn containment_cycles_are_rejected() {
    let mut model = example_model();
    component(&mut model, "crossing").parent = Some("crossing".into());
    rejects(&model, "L2R-S005");
}

#[test]
fn a_second_root_is_rejected() {
    let mut model = example_model();
    let mut extra = component(&mut model, "crossing").clone();
    extra.id = "second".into();
    extra.rust_module = "second".into();
    model.components.push(extra);
    rejects(&model, "L2R-S006");
}

#[test]
fn unknown_profiles_are_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").profile = "transition-core/v9".into();
    rejects(&model, "L2R-S007");
}

#[test]
fn a_machine_without_behavior_is_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").behavior = None;
    rejects(&model, "L2R-S008");
}

#[test]
fn a_composite_with_behavior_is_rejected() {
    let mut model = example_model();
    let borrowed = behavior(&mut model, "signal").clone();
    component(&mut model, "crossing").behavior = Some(borrowed);
    rejects(&model, "L2R-S008");
}

#[test]
fn sibling_module_collisions_are_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").rust_module = "request_latch".into();
    rejects(&model, "L2R-S009");
}

#[test]
fn reserved_module_names_are_rejected() {
    for reserved in ["imp", "routes", "match"] {
        let mut model = example_model();
        component(&mut model, "signal").rust_module = reserved.into();
        rejects(&model, "L2R-S010");
    }
}

#[test]
fn empty_responsibilities_are_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").responsibility = "  ".into();
    rejects(&model, "L2R-S011");
}

#[test]
fn control_characters_in_notes_are_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").notes = Some("ring\u{7}".into());
    rejects(&model, "L2R-S012");
}

#[test]
fn escaping_paths_are_rejected() {
    for path in ["../elsewhere", "/abs/out", "a/../../b", ".git"] {
        let mut model = example_model();
        model.system.output_dir = path.into();
        rejects(&model, "L2R-S013");
    }
}

#[test]
fn zero_budgets_are_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").implementation_budget_lines = Some(0);
    rejects(&model, "L2R-S014");
}

#[test]
fn shared_state_ownership_is_rejected() {
    let mut model = example_model();
    behavior(&mut model, "request_latch").owned_state = "Crossing.Signal.State".into();
    rejects(&model, "L2R-O001");
}

#[test]
fn unsupported_inventory_schema_is_rejected() {
    let mut inventory = example_inventory();
    inventory.schema = "other".into();
    assert!(codes(&example_model(), &inventory).contains(&"L2R-L001"));
}

#[test]
fn malformed_lean_names_are_rejected() {
    let mut model = example_model();
    behavior(&mut model, "signal").step = "Crossing..step".into();
    rejects(&model, "L2R-L002");
}

#[test]
fn unknown_lean_names_are_rejected() {
    let mut model = example_model();
    behavior(&mut model, "signal")
        .invariants
        .push("Crossing.Signal.always_safe".into());
    rejects(&model, "L2R-L003");
}

#[test]
fn wrong_declaration_kinds_are_rejected() {
    let mut model = example_model();
    // A definition where a theorem is required.
    behavior(&mut model, "signal")
        .invariants
        .push("Crossing.Signal.vehiclesMayMove".into());
    rejects(&model, "L2R-L004");
}

#[test]
fn enumerations_with_fields_are_rejected() {
    let mut inventory = example_inventory();
    let state = inventory
        .declarations
        .iter_mut()
        .find(|d| d.name == "Crossing.Signal.State")
        .unwrap();
    state.constructors[0].arity = 1;
    assert!(codes(&example_model(), &inventory).contains(&"L2R-L005"));
}

#[test]
fn transition_signature_mismatches_are_rejected() {
    let mut model = example_model();
    // Signal's step does not operate on the latch's state.
    behavior(&mut model, "request_latch").step = "Crossing.Signal.step".into();
    rejects(&model, "L2R-L006");
}

#[test]
fn colliding_variant_names_are_rejected() {
    let mut inventory = example_inventory();
    let state = inventory
        .declarations
        .iter_mut()
        .find(|d| d.name == "Crossing.Signal.State")
        .unwrap();
    let mut twin = state.constructors[0].clone();
    twin.name = "Crossing.Signal.State.vehicles_green".into();
    state.constructors.push(twin);
    assert!(codes(&example_model(), &inventory).contains(&"L2R-L007"));
}

fn edge(id: &str, from: &str, to: &str, translation: &str) -> MessageEdge {
    MessageEdge {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        translation: translation.into(),
    }
}

#[test]
fn duplicate_message_ids_are_rejected() {
    let mut model = example_model();
    model.messages[1].id = model.messages[0].id.clone();
    rejects(&model, "L2R-M001");
}

#[test]
fn messages_to_unknown_components_are_rejected() {
    let mut model = example_model();
    model.messages.push(edge(
        "to_nowhere",
        "signal",
        "nowhere",
        "Crossing.Routes.signalToLatch",
    ));
    rejects(&model, "L2R-M002");
}

#[test]
fn messages_to_composites_are_rejected() {
    let mut model = example_model();
    model.messages.push(edge(
        "to_parent",
        "signal",
        "crossing",
        "Crossing.Routes.signalToLatch",
    ));
    rejects(&model, "L2R-M003");
}

#[test]
fn self_messages_are_rejected() {
    let mut model = example_model();
    model.messages.push(edge(
        "loop_back",
        "signal",
        "signal",
        "Crossing.Routes.signalToLatch",
    ));
    rejects(&model, "L2R-M004");
}

#[test]
fn messages_across_subtrees_are_rejected() {
    let mut model = example_model();
    // Move the latch into a nested composite: the edges now cross a boundary.
    model.components.push(Component {
        id: "kerb".into(),
        name: "Kerb unit".into(),
        parent: Some("crossing".into()),
        profile: "composite/v0".into(),
        responsibility: "Groups kerbside equipment.".into(),
        rust_module: "kerb".into(),
        notes: None,
        implementation_budget_lines: None,
        behavior: None,
    });
    component(&mut model, "request_latch").parent = Some("kerb".into());
    rejects(&model, "L2R-M005");
}

#[test]
fn ambiguous_parallel_edges_are_rejected() {
    let mut model = example_model();
    model.messages.push(edge(
        "again",
        "request_latch",
        "signal",
        "Crossing.Routes.latchToSignal",
    ));
    rejects(&model, "L2R-M006");
}

#[test]
fn mistyped_translations_are_rejected() {
    let mut model = example_model();
    model.messages[0].translation = "Crossing.Routes.signalToLatch".into();
    rejects(&model, "L2R-L006");
}

#[test]
fn every_finding_is_reported_at_once() {
    let mut model = example_model();
    component(&mut model, "signal").rust_module = "Signal".into();
    behavior(&mut model, "request_latch").step = "Crossing.Nope.step".into();
    let found = codes(&model, &example_inventory());
    assert!(
        found.contains(&"L2R-S002") && found.contains(&"L2R-L003"),
        "{found:?}"
    );
}

#[test]
fn a_kind_change_in_lean_invalidates_the_model() {
    // If the Lean project turns a theorem into an axiom, the model no longer validates.
    let mut inventory = example_inventory();
    let theorem = inventory
        .declarations
        .iter_mut()
        .find(|d| d.name == "Crossing.Signal.no_conflicting_release")
        .unwrap();
    theorem.kind = DeclKind::Axiom;
    assert!(codes(&example_model(), &inventory).contains(&"L2R-L004"));
}
