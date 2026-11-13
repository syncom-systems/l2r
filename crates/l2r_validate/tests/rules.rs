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
    model.schema = "l2r.architecture/v99".into();
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
        mailbox_capacity: None,
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

// ---------------------------------------------------------------------------
// composite/v1: ports, capacities and the rules that keep them honest.
// ---------------------------------------------------------------------------

use l2r_testing::{example_v1_inventory, example_v1_model};

fn v1_codes(model: &Model) -> Vec<&'static str> {
    codes(model, &example_v1_inventory())
}

fn v1_rejects(model: &Model, code: &str) {
    let found = v1_codes(model);
    assert!(found.contains(&code), "expected {code}, got {found:?}");
}

#[test]
fn the_v1_example_is_valid_and_resolves_its_port_types() {
    let validated = validate(&example_v1_model(), &example_v1_inventory()).expect("valid");
    let button = validated.port_message("button").expect("resolved");
    assert_eq!(button.lean_name, "CrossingV1.Ports.ButtonEvent");
    assert_eq!(button.variants[0].rust_name, "Pressed");
}

#[test]
fn a_v0_document_cannot_use_v1_features() {
    // Schema `v0` means what it always meant; it does not silently gain ports.
    let mut model = example_v1_model();
    model.schema = "l2r.architecture/v0".into();
    v1_rejects(&model, "L2R-S015");
    let mut only_capacity = example_model();
    component(&mut only_capacity, "signal").mailbox_capacity = Some(2);
    rejects(&only_capacity, "L2R-S015");
}

#[test]
fn v0_models_are_still_valid_under_the_v1_schema() {
    let mut model = example_model();
    model.schema = "l2r.architecture/v1".into();
    assert!(codes(&model, &example_inventory()).is_empty());
}

#[test]
fn every_child_of_a_v1_composite_must_state_a_capacity() {
    let mut model = example_v1_model();
    component(&mut model, "signal").mailbox_capacity = None;
    v1_rejects(&model, "L2R-S017");
    component(&mut model, "signal").mailbox_capacity = Some(0);
    v1_rejects(&model, "L2R-S017");
    component(&mut model, "signal").mailbox_capacity = Some(10_000);
    v1_rejects(&model, "L2R-S017");
}

#[test]
fn capacity_outside_a_v1_composite_is_rejected() {
    let mut model = example_v1_model();
    component(&mut model, "crossing").mailbox_capacity = Some(2);
    v1_rejects(&model, "L2R-S018");
}

#[test]
fn port_structure_is_checked() {
    let mut model = example_v1_model();
    let mut dup = model.ports[0].clone();
    dup.translation = model.ports[0].translation.clone();
    model.ports.push(dup);
    v1_rejects(&model, "L2R-P001");

    let mut model = example_v1_model();
    model.ports[0].composite = "signal".into();
    v1_rejects(&model, "L2R-P002");

    let mut model = example_v1_model();
    model.ports[0].to = Some("crossing".into());
    v1_rejects(&model, "L2R-P003");

    let mut model = example_v1_model();
    model.ports.clear();
    v1_rejects(&model, "L2R-P004");
}

#[test]
fn a_port_must_translate_its_message_into_its_targets_input() {
    // The wrong translation (it produces the latch's input, not the signal's)
    // is the typed-interface guarantee in action.
    let mut model = example_v1_model();
    let timer = model.ports.iter_mut().find(|p| p.id == "timer").unwrap();
    timer.translation = Some("CrossingV1.Ports.buttonToLatch".into());
    v1_rejects(&model, "L2R-L006");
    let mut model = example_v1_model();
    model.ports[0].message = "CrossingV1.Latch.Input".into();
    v1_rejects(&model, "L2R-L006");
}

#[test]
fn a_machine_whose_outputs_nobody_consumes_is_rejected() {
    // composite/v1 never discards silently and has no output ports yet.
    let mut model = example_v1_model();
    model.messages.retain(|m| m.from != "signal");
    v1_rejects(&model, "L2R-M007");
}

#[test]
fn a_v1_composite_must_be_the_root_and_hold_only_machines() {
    let mut model = example_v1_model();
    model.components.push(Component {
        id: "outer".into(),
        name: "Outer".into(),
        parent: None,
        profile: "composite/v0".into(),
        responsibility: "Holds the crossing.".into(),
        rust_module: "outer".into(),
        notes: None,
        implementation_budget_lines: None,
        behavior: None,
        mailbox_capacity: None,
    });
    component(&mut model, "crossing").parent = Some("outer".into());
    v1_rejects(&model, "L2R-S016");
}

#[test]
fn validated_carries_the_typed_schema_and_profiles() {
    use l2r_model::{Profile, Schema};
    let validated = validate(&example_model(), &example_inventory()).expect("valid");
    assert_eq!(validated.schema(), Schema::V0);
    assert_eq!(validated.profile("crossing"), Profile::CompositeV0);
    assert_eq!(validated.profile("signal"), Profile::TransitionCoreV0);
    let v1 = validate(&example_v1_model(), &example_v1_inventory()).expect("valid");
    assert_eq!(v1.schema(), Schema::V1);
    assert_eq!(v1.profile("crossing"), Profile::CompositeV1);
}

#[test]
fn each_v1_feature_alone_needs_schema_v1() {
    // composite/v1 alone (no ports, no capacities) under v0.
    let mut profile_only = example_model();
    component(&mut profile_only, "crossing").profile = "composite/v1".into();
    rejects(&profile_only, "L2R-S015");
    // a mailbox capacity alone under v0.
    let mut capacity_only = example_model();
    component(&mut capacity_only, "signal").mailbox_capacity = Some(1);
    rejects(&capacity_only, "L2R-S015");
}

#[test]
fn port_fields_of_v2_are_refused_under_v0_and_unknown_schemas() {
    for schema in ["l2r.architecture/v0", "l2r.architecture/v9"] {
        let mut model = example_v1_model();
        model.schema = schema.into();
        model.ports[0].direction = Some("in".into());
        let found = v1_codes(&model);
        assert!(found.contains(&"L2R-S015"), "{schema}: {found:?}");
    }
}
