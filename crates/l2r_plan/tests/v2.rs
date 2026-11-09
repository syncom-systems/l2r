//! Planning a hierarchical (`v2`) model: deterministic, complete, and the
//! derived Lean network is the committed one.

use l2r_model::{Inventory, Model};
use l2r_plan::{GeneratorIdentity, Ownership, Plan, lean_composition, plan};
use l2r_testing::{relay_inventory, relay_model, tank_inventory, tank_model};
use l2r_validate::validate;

fn plan_of(model: &Model, inventory: &Inventory) -> Plan {
    let validated = validate(model, inventory).expect("valid");
    plan(&validated, &GeneratorIdentity::current()).expect("plans")
}

fn tank_plan() -> Plan {
    plan_of(&tank_model(), &tank_inventory())
}

#[test]
fn the_tank_station_plans_the_expected_files() {
    let plan = tank_plan();
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
            ("src/lib.rs", Generated),
            ("src/station/alarm/contract_tests.rs", Generated),
            ("src/station/alarm/imp.rs", Implementation),
            ("src/station/alarm/mod.rs", Generated),
            ("src/station/controller/contract_tests.rs", Generated),
            ("src/station/controller/imp.rs", Implementation),
            ("src/station/controller/mod.rs", Generated),
            ("src/station/mod.rs", Generated),
            ("src/station/routes.rs", Implementation),
            ("src/station/sensing/filter/contract_tests.rs", Generated),
            ("src/station/sensing/filter/imp.rs", Implementation),
            ("src/station/sensing/filter/mod.rs", Generated),
            ("src/station/sensing/mod.rs", Generated),
            ("src/station/sensing/routes.rs", Implementation),
            ("src/types.rs", Generated),
        ]
    );
    assert_eq!(
        plan.profiles,
        ["composite/v2", "crate-root/v1", "transition-core/v1"]
    );
}

#[test]
fn planning_is_deterministic_and_order_independent() {
    let mut shuffled = tank_model();
    shuffled.components.reverse();
    shuffled.messages.reverse();
    shuffled.ports.reverse();
    assert_eq!(
        tank_plan().digest(),
        plan_of(&shuffled, &tank_inventory()).digest()
    );
    assert_eq!(tank_plan(), tank_plan());
}

#[test]
fn every_input_that_shapes_the_output_changes_the_digest() {
    let base = tank_plan().digest();
    // A capacity is part of the behaviour.
    let mut model = tank_model();
    model
        .ports
        .iter_mut()
        .find(|p| p.id == "valve_command")
        .unwrap()
        .capacity = Some(3);
    assert_ne!(base, plan_of(&model, &tank_inventory()).digest());
    // A field added to a Lean type changes the generated mirror.
    let mut inventory = tank_inventory();
    let sample = inventory
        .declarations
        .iter_mut()
        .find(|d| d.name == "Tank.Ports.Sample")
        .unwrap();
    let mut extra = sample.constructors[0].fields[0].clone();
    extra.name = "second".into();
    sample.constructors[0].fields.push(extra);
    sample.constructors[0].arity = 2;
    let changed = plan_of(&tank_model(), &inventory);
    assert_ne!(base, changed.digest());
    let types = changed.artifact("src/types.rs").unwrap();
    assert!(
        types.content.contains("pub second: u8,"),
        "{}",
        types.content
    );
}

#[test]
fn each_lean_type_has_exactly_one_rust_type() {
    let plan = tank_plan();
    let types = &plan.artifact("src/types.rs").unwrap().content;
    // `Tank.Level` is used by the filter, the controller, a port and two routes.
    assert_eq!(types.matches("pub enum Level {").count(), 1);
    // Machines alias the mirrors; they do not define types of their own.
    let filter = &plan
        .artifact("src/station/sensing/filter/mod.rs")
        .unwrap()
        .content;
    assert!(filter.contains("pub type State = crate::types::tank::filter::State;"));
    assert!(!filter.contains("pub enum") && !filter.contains("pub struct"));
    // Data survives: a constructor field, a structure field, an Option.
    assert!(types.contains("        Sample(u8),"));
    assert!(types.contains("            pub streak: u8,"));
    assert!(types.contains("            pub count: Option<u8>,"));
}

#[test]
fn only_the_root_composite_schedules() {
    let plan = tank_plan();
    let root = &plan.artifact("src/station/mod.rs").unwrap().content;
    let nested = &plan.artifact("src/station/sensing/mod.rs").unwrap().content;
    assert!(root.contains("pub struct System {"));
    assert!(!nested.contains("System"));
    // The nested machine is scheduled by the root, in the flat network.
    assert!(root.contains("fn attempt_filter(&mut self) -> Attempt {"));
    assert!(root.contains("sensing::filter::step(self.filter_state, input)"));
    // Output ports are bounded outboxes the environment empties.
    assert!(root.contains("pub fn take_valve_command(&mut self) -> Option<ValveCommandMessage> {"));
    assert!(root.contains("pub const VALVE_COMMAND_CAPACITY: usize = 2;"));
}

#[test]
fn a_route_through_several_boundaries_composes_its_translations_in_order() {
    let plan = plan_of(&relay_model(), &relay_inventory());
    let root = &plan.artifact("src/plant/mod.rs").unwrap().content;
    let expected = "\
fn source_route_0(output: SourceOutput) -> Option<SinkInput> {
    match zone_a::cell::routes::source_to_cell(output) {
        Some(message) => match zone_a::routes::cell_to_zone(message) {
            Some(message) => match routes::zone_to_b(message) {
                Some(message) => Some(zone_b::routes::port_zone_b_in(message)),
                None => None,
            },
            None => None,
        },
        None => None,
    }
}
";
    assert!(root.contains(expected), "{root}");
    // Two routes to one receiver: room is needed for both, before anything changes.
    assert!(root.contains("if self.sink_inbox.len() + to_sink > SINK_CAPACITY {"));
    // A chain of forwarded input ports.
    let send = "\
        let message = routes::port_poke(message);
        let message = zone_a::routes::port_zone_a_in(message);
        let input = zone_a::cell::routes::port_cell_in(message);
";
    assert!(root.contains(send), "{root}");
}

const TANK_DERIVED: &str =
    include_str!("../../../examples/tank_station/composition/L2RDerived/tank_station.lean");
const RELAY_DERIVED: &str =
    include_str!("../../../examples/relay/composition/L2RDerived/relay.lean");
const TANK_APPROVAL: &str =
    include_str!("../../../examples/tank_station/architecture.approval.json");
const SEMANTICS: &str = include_str!("../../../lean/L2R/Async1.lean");
const SCHEDULER: &str = include_str!("../../../lean/L2R/Async1Ports.lean");

#[test]
fn the_committed_derived_networks_are_current() {
    for (model, inventory, committed, path) in [
        (
            tank_model(),
            tank_inventory(),
            TANK_DERIVED,
            "examples/tank_station",
        ),
        (
            relay_model(),
            relay_inventory(),
            RELAY_DERIVED,
            "examples/relay",
        ),
    ] {
        let validated = validate(&model, &inventory).expect("valid");
        let fresh = lean_composition(&validated).expect("derives");
        assert!(
            fresh == committed,
            "{path}/composition/L2RDerived is stale; regenerate it with \
             `l2r lean-composition {path}/architecture.toml`\n--- fresh ---\n{fresh}"
        );
    }
}

fn header(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix(&format!("-- {key}: ")))
        .unwrap_or_else(|| panic!("no `-- {key}:` header line"))
        .to_string()
}

#[test]
fn the_derived_network_names_exactly_what_it_was_derived_from() {
    let record: serde_json::Value = serde_json::from_str(TANK_APPROVAL).expect("approval parses");
    let field = |key: &str| record[key].as_str().expect("string field").to_string();
    assert_eq!(header(TANK_DERIVED, "model"), field("model_digest"));
    assert_eq!(header(TANK_DERIVED, "inventory"), field("inventory_digest"));
    assert_eq!(header(TANK_DERIVED, "generator"), field("generator"));
    assert_eq!(
        header(TANK_DERIVED, "semantics"),
        format!(
            "l2r.async/v1 {}",
            l2r_model::digest::sha256(SEMANTICS.as_bytes())
        )
    );
    assert_eq!(
        header(TANK_DERIVED, "scheduler"),
        format!(
            "{} {}",
            l2r_plan::PORTS_SCHEDULER_VERSION,
            l2r_model::digest::sha256(SCHEDULER.as_bytes())
        )
    );
    assert!(TANK_DERIVED.starts_with(&format!(
        "-- @generated by L2R ({}).",
        l2r_plan::LEAN_COMPOSITION_FORMAT_V2
    )));
}

#[test]
fn the_derived_network_restates_no_behaviour() {
    // Outboxes are children of the network; routes compose the Lean
    // translations in the order the generated scheduler applies them.
    assert!(TANK_DERIVED.contains("  | ⟨3, _⟩ => L2R.Async1.outboxStep Tank.Ports.AlarmNotice"));
    assert!(TANK_DERIVED.contains(
        "((Tank.Routes.filterToLevel o).bind Tank.Routes.levelToController).toList.map (fun i => ⟨⟨1, by decide⟩, i⟩)"
    ));
    assert!(TANK_DERIVED.contains("def machines : Nat := 3"));
    assert!(RELAY_DERIVED.contains(
        "fun m => Relay.Ports.cellToSource (Relay.Ports.zoneToCell (Relay.Ports.plantToZone m))"
    ));
}
