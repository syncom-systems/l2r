//! Each rule of model schema `v2` rejects a minimal violation of the tank
//! station model, and the flat network is what the hierarchy says it is.

use l2r_model::{Component, DeclKind, Declaration, Inventory, MessageEdge, Model, Port};
use l2r_testing::{
    example_v1_inventory, example_v1_model, relay_inventory, relay_model, tank_inventory,
    tank_model,
};
use l2r_validate::flat::{HopKind, Target};
use l2r_validate::{Diagnostic, validate};

fn codes(model: &Model, inventory: &Inventory) -> Vec<&'static str> {
    match validate(model, inventory) {
        Ok(_) => vec![],
        Err(diagnostics) => diagnostics.iter().map(|d: &Diagnostic| d.code).collect(),
    }
}

fn rejects(model: &Model, code: &str) {
    let found = codes(model, &tank_inventory());
    assert!(found.contains(&code), "expected {code}, got {found:?}");
}

fn component<'a>(model: &'a mut Model, id: &str) -> &'a mut Component {
    model
        .components
        .iter_mut()
        .find(|c| c.id == id)
        .expect("component exists")
}

fn port<'a>(model: &'a mut Model, id: &str) -> &'a mut Port {
    model
        .ports
        .iter_mut()
        .find(|p| p.id == id)
        .expect("port exists")
}

fn edge<'a>(model: &'a mut Model, id: &str) -> &'a mut MessageEdge {
    model
        .messages
        .iter_mut()
        .find(|e| e.id == id)
        .expect("edge exists")
}

#[test]
fn the_tank_station_is_valid_and_flattens_to_its_machines() {
    let validated = validate(&tank_model(), &tank_inventory()).expect("valid");
    let flat = validated.flat().expect("a v2 model has a flat network");
    let leaves: Vec<&str> = flat.leaves.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(leaves, ["alarm", "controller", "filter"]);
    let sinks: Vec<&str> = flat.sinks.iter().map(|s| s.port.as_str()).collect();
    assert_eq!(sinks, ["alarm_notice", "valve_command"]);
    assert_eq!(flat.sink_index("valve_command"), Some(4));

    // The filter is inside `sensing`; its output crosses that boundary through
    // the `level_changed` port and then an edge of the station.
    let filter = &flat.leaves[2];
    assert_eq!(filter.routes.len(), 1);
    let hops: Vec<(&str, &str)> = filter.routes[0]
        .hops
        .iter()
        .map(|h| (h.id.as_str(), h.owner.as_str()))
        .collect();
    assert_eq!(
        hops,
        [
            ("filter_to_level", "sensing"),
            ("level_to_controller", "station")
        ]
    );
    assert_eq!(filter.routes[0].target, Target::Leaf("controller".into()));

    // The station's `sample` port is forwarded into `sensing`.
    let sample = flat.inputs.iter().find(|i| i.port == "sample").unwrap();
    assert_eq!(sample.target, "filter");
    let via: Vec<&str> = sample.hops.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(via, ["sample", "reading"]);
    assert!(sample.hops.iter().all(|h| h.kind == HopKind::Port));
}

#[test]
fn data_types_are_resolved_once_per_lean_type() {
    let validated = validate(&tank_model(), &tank_inventory()).expect("valid");
    let level = validated.data_type("Tank.Level").expect("shared type");
    assert_eq!(level.rust_path(), "crate::types::tank::Level");
    let state = validated.data_type("Tank.Filter.State").unwrap();
    assert_eq!(state.rust_path(), "crate::types::tank::filter::State");
    assert_eq!(state.dependencies(), ["Tank.Level", "Tank.Level"]);
    // A type only fields mention is resolved too.
    assert!(validated.data_type("Tank.Controller.Command").is_some());
    // A type nothing in the model mentions is not.
    assert!(validated.data_type("Tank.Ports.Nothing").is_none());
}

#[test]
fn the_relay_fixture_has_two_routes_to_one_receiver() {
    let validated = validate(&relay_model(), &relay_inventory()).expect("valid");
    let flat = validated.flat().unwrap();
    let source = flat.leaves.iter().find(|l| l.id == "source").unwrap();
    let targets: Vec<&Target> = source.routes.iter().map(|r| &r.target).collect();
    assert_eq!(
        targets,
        [
            &Target::Leaf("sink".into()),
            &Target::Sink("tap".into()),
            &Target::Leaf("sink".into())
        ]
    );
    // edge, edge, edge, then the receiving composite's input port.
    let kinds: Vec<HopKind> = source.routes[0].hops.iter().map(|h| h.kind).collect();
    assert_eq!(
        kinds,
        [HopKind::Edge, HopKind::Edge, HopKind::Edge, HopKind::Port]
    );
    let poke = &flat.inputs[0];
    assert_eq!((poke.hops.len(), poke.target.as_str()), (3, "source"));
}

#[test]
fn v2_profiles_and_older_schemas_do_not_mix() {
    // A v2 model cannot use an older profile.
    let mut model = tank_model();
    component(&mut model, "alarm").profile = "transition-core/v0".into();
    rejects(&model, "L2R-S019");
    // An older schema cannot use a v2 profile.
    let mut model = example_v1_model();
    model
        .components
        .iter_mut()
        .find(|c| c.id == "signal")
        .unwrap()
        .profile = "transition-core/v1".into();
    assert!(codes(&model, &example_v1_inventory()).contains(&"L2R-S019"));
}

#[test]
fn a_v2_model_needs_an_inventory_with_fields() {
    let mut inventory = tank_inventory();
    inventory.schema = "l2r.lean-inventory/v0".into();
    assert!(codes(&tank_model(), &inventory).contains(&"L2R-L001"));
}

#[test]
fn machine_and_composite_shapes_are_checked() {
    let mut model = tank_model();
    component(&mut model, "alarm").behavior = None;
    rejects(&model, "L2R-S008");

    let mut model = tank_model();
    component(&mut model, "alarm").mailbox_capacity = None;
    rejects(&model, "L2R-S017");
    component(&mut model, "alarm").mailbox_capacity = Some(65);
    rejects(&model, "L2R-S017");

    let mut model = tank_model();
    component(&mut model, "sensing").mailbox_capacity = Some(1);
    rejects(&model, "L2R-S018");

    let mut model = tank_model();
    component(&mut model, "station").rust_module = "types".into();
    rejects(&model, "L2R-S010");

    let mut model = tank_model();
    component(&mut model, "alarm").id = "take".into();
    rejects(&model, "L2R-S010");
}

#[test]
fn port_rules() {
    let mut model = tank_model();
    port(&mut model, "sample").direction = None;
    rejects(&model, "L2R-P007");

    let mut model = tank_model();
    port(&mut model, "sample").composite = "alarm".into();
    rejects(&model, "L2R-P002");

    // An `in` port feeds a child machine or a child composite's `in` port.
    let mut model = tank_model();
    port(&mut model, "sample").to = Some("filter".into());
    rejects(&model, "L2R-P003");
    let mut model = tank_model();
    port(&mut model, "sample").to = Some("level_changed".into());
    rejects(&model, "L2R-P003");
    let mut model = tank_model();
    port(&mut model, "sample").to = None;
    rejects(&model, "L2R-P003");

    // No in port on the root: nothing can enter.
    let mut model = tank_model();
    model
        .ports
        .retain(|p| p.composite != "station" || p.is_out());
    rejects(&model, "L2R-P004");

    // A port and a component share the edge namespace.
    let mut model = tank_model();
    port(&mut model, "reading").id = "alarm".into();
    rejects(&model, "L2R-P006");

    // Capacity belongs to the root's out ports, and each must state one.
    let mut model = tank_model();
    port(&mut model, "valve_command").capacity = None;
    rejects(&model, "L2R-P008");
    let mut model = tank_model();
    port(&mut model, "level_changed").capacity = Some(1);
    rejects(&model, "L2R-P008");
    let mut model = tank_model();
    port(&mut model, "sample").capacity = Some(1);
    rejects(&model, "L2R-P008");

    // An out port has no target of its own.
    let mut model = tank_model();
    port(&mut model, "valve_command").to = Some("controller".into());
    rejects(&model, "L2R-P009");
}

#[test]
fn edge_rules() {
    // Nothing reaches into a subtree: `filter` is not visible in `station`.
    let mut model = tank_model();
    edge(&mut model, "controller_to_alarm").to = "filter".into();
    rejects(&model, "L2R-M005");

    // Nor out of one: the filter cannot write to the station's port directly.
    let mut model = tank_model();
    edge(&mut model, "filter_to_level").to = "valve_command".into();
    rejects(&model, "L2R-M005");

    let mut model = tank_model();
    edge(&mut model, "controller_to_alarm").to = "nowhere".into();
    rejects(&model, "L2R-M002");

    // A composite is not an endpoint; its ports are.
    let mut model = tank_model();
    edge(&mut model, "controller_to_alarm").to = "sensing".into();
    rejects(&model, "L2R-M003");

    // An `in` port forwards by its own `to`, never by an edge.
    let mut model = tank_model();
    edge(&mut model, "controller_to_alarm").from = "operator".into();
    rejects(&model, "L2R-M003");

    // The root's out ports leave the system.
    let mut model = tank_model();
    edge(&mut model, "controller_to_alarm").from = "valve_command".into();
    rejects(&model, "L2R-M003");

    let mut model = tank_model();
    edge(&mut model, "controller_to_alarm").to = "controller".into();
    rejects(&model, "L2R-M004");

    let mut model = tank_model();
    let mut second = model.messages[0].clone();
    second.id = "second".into();
    model.messages.push(second);
    rejects(&model, "L2R-M006");
}

#[test]
fn nothing_is_silently_discarded() {
    // A machine whose outputs go nowhere.
    let mut model = tank_model();
    model.messages.retain(|e| e.from != "alarm");
    rejects(&model, "L2R-M007");
    // A nested out port nobody listens to.
    let mut model = tank_model();
    model.messages.retain(|e| e.from != "level_changed");
    rejects(&model, "L2R-M007");
}

#[test]
fn an_output_may_not_route_back_to_its_own_machine() {
    // sink → zone_b_out → (plant) → zone_b_in → sink: a well-typed loop.
    let mut relay = relay_model();
    relay.messages.push(MessageEdge {
        id: "echo".into(),
        from: "zone_b_out".into(),
        to: "zone_b_in".into(),
        translation: "Relay.Routes.echo".into(),
    });
    let mut inventory = relay_inventory();
    inventory.declarations.push(Declaration {
        name: "Relay.Routes.echo".into(),
        kind: DeclKind::Definition,
        type_: "Relay.Ports.Report → Option Relay.Token".into(),
        constructors: vec![],
        structure: false,
        params: 0,
        indices: 0,
    });
    assert_eq!(codes(&relay, &inventory), ["L2R-M008"]);
}

#[test]
fn generated_names_must_be_distinct() {
    // `a_1` and `a1` would both generate the type prefix `A1`.
    let mut model = tank_model();
    component(&mut model, "alarm").id = "a_1".into();
    component(&mut model, "controller").id = "a1".into();
    rejects(&model, "L2R-S020");
    // An edge `port_reading` would collide with the function of port `reading`.
    let mut model = tank_model();
    edge(&mut model, "filter_to_level").id = "port_reading".into();
    rejects(&model, "L2R-S020");
}

#[test]
fn translations_are_type_checked_across_boundaries() {
    // The edge out of `sensing` must take the port's message type.
    let mut model = tank_model();
    edge(&mut model, "level_to_controller").translation = "Tank.Routes.filterToLevel".into();
    rejects(&model, "L2R-L006");
    // A forwarded port must produce the next port's message type.
    let mut model = tank_model();
    port(&mut model, "sample").translation = Some("Tank.Ports.readingToFilter".into());
    rejects(&model, "L2R-L006");
}

#[test]
fn unsupported_data_types_are_rejected_not_approximated() {
    let field_type = |inventory: &mut Inventory, ty: &str| {
        let state = inventory
            .declarations
            .iter_mut()
            .find(|d| d.name == "Tank.Alarm.State")
            .unwrap();
        state.constructors[0].fields[1].type_ = ty.into();
    };
    for unsupported in ["Nat", "List UInt8", "Option (Option UInt8)", "String"] {
        let mut inventory = tank_inventory();
        field_type(&mut inventory, unsupported);
        let found = codes(&tank_model(), &inventory);
        assert!(found.contains(&"L2R-L005"), "{unsupported}: {found:?}");
    }
    // A recursive type has no finite representation.
    let mut inventory = tank_inventory();
    field_type(&mut inventory, "Tank.Alarm.State");
    assert!(codes(&tank_model(), &inventory).contains(&"L2R-L008"));
    // A parameterized type is not plain data.
    let mut inventory = tank_inventory();
    inventory
        .declarations
        .iter_mut()
        .find(|d| d.name == "Tank.Level")
        .unwrap()
        .params = 1;
    assert!(codes(&tank_model(), &inventory).contains(&"L2R-L005"));
    // A field type must itself be in the inventory.
    let mut inventory = tank_inventory();
    field_type(&mut inventory, "Tank.Missing");
    assert!(codes(&tank_model(), &inventory).contains(&"L2R-L003"));
}

#[test]
fn every_finding_is_reported_in_one_run() {
    let mut model = tank_model();
    port(&mut model, "sample").direction = None;
    component(&mut model, "alarm").mailbox_capacity = None;
    edge(&mut model, "controller_to_alarm").to = "nowhere".into();
    let found = codes(&model, &tank_inventory());
    for code in ["L2R-P007", "L2R-S017", "L2R-M002"] {
        assert!(found.contains(&code), "expected {code}, got {found:?}");
    }
}
