//! Planning a hierarchical (`v2`) model: deterministic, complete, and the
//! derived Lean network is the committed one.

use l2r_model::{Inventory, Model};
use l2r_plan::{
    ConformanceSide, GeneratorIdentity, Ownership, Plan, conformance, lean_composition, plan,
};
use l2r_testing::{
    relay_inventory, relay_model, starter_inventory, starter_model, tank_inventory, tank_model,
};
use l2r_validate::validate;

fn plan_of(model: &Model, inventory: &Inventory) -> Plan {
    let validated = validate(model, inventory).expect("valid");
    plan(&validated, &GeneratorIdentity::current()).expect("plans")
}

fn tank_plan() -> Plan {
    plan_of(&tank_model(), &tank_inventory())
}

/// The planned content of one file.
fn content(plan: &Plan, path: &str) -> String {
    plan.artifact(path)
        .unwrap_or_else(|| panic!("`{path}` is not planned"))
        .content
        .clone()
}

/// The root module of the tank station: the one that holds the `System`.
fn tank_root() -> String {
    content(&tank_plan(), "src/station/mod.rs")
}

/// The root module of the relay: its routes cross several boundaries.
fn relay_root() -> String {
    content(
        &plan_of(&relay_model(), &relay_inventory()),
        "src/plant/mod.rs",
    )
}

/// The fragments that `text` does not contain; empty when it contains them all.
fn missing<'a>(text: &str, fragments: &[&'a str]) -> Vec<&'a str> {
    fragments
        .iter()
        .copied()
        .filter(|fragment| !text.contains(fragment))
        .collect()
}

const NONE_MISSING: [&str; 0] = [];

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
fn a_capacity_changes_the_digest() {
    // A capacity is part of the behaviour.
    let mut model = tank_model();
    model
        .ports
        .iter_mut()
        .find(|p| p.id == "valve_command")
        .unwrap()
        .capacity = Some(3);
    assert_ne!(
        tank_plan().digest(),
        plan_of(&model, &tank_inventory()).digest()
    );
}

/// The tank station's inventory with more fields, of the given names, on the
/// constructor of `Tank.Ports.Sample`; each is a copy of its first field.
fn tank_inventory_with_more_sample_fields(names: impl IntoIterator<Item = String>) -> Inventory {
    let mut inventory = tank_inventory();
    let sample = inventory
        .declarations
        .iter_mut()
        .find(|d| d.name == "Tank.Ports.Sample")
        .unwrap();
    let constructor = &mut sample.constructors[0];
    let field = constructor.fields[0].clone();
    for name in names {
        let mut added = field.clone();
        added.name = name;
        constructor.fields.push(added);
        constructor.arity += 1;
    }
    inventory
}

#[test]
fn a_field_added_to_a_lean_type_changes_the_digest_and_the_mirror() {
    let inventory = tank_inventory_with_more_sample_fields(["second".to_string()]);
    let changed = plan_of(&tank_model(), &inventory);
    assert_ne!(tank_plan().digest(), changed.digest());
    let types = content(&changed, "src/types.rs");
    assert!(types.contains("pub second: u8,"), "{types}");
}

#[test]
fn each_lean_type_has_exactly_one_rust_type() {
    // `Tank.Level` is used by the filter, the controller, a port and two routes.
    let types = content(&tank_plan(), "src/types.rs");
    assert_eq!(types.matches("pub enum Level {").count(), 1);
}

#[test]
fn machines_alias_the_mirrors_and_define_no_types_of_their_own() {
    let filter = content(&tank_plan(), "src/station/sensing/filter/mod.rs");
    assert!(filter.contains("pub type State = crate::types::tank::filter::State;"));
    assert!(!filter.contains("pub enum") && !filter.contains("pub struct"));
}

#[test]
fn data_survives_in_the_mirrors() {
    // A constructor field, a structure field, an Option.
    let types = content(&tank_plan(), "src/types.rs");
    assert_eq!(
        missing(
            &types,
            &[
                "        Sample(u8),",
                "            pub streak: u8,",
                "            pub count: Option<u8>,",
            ]
        ),
        NONE_MISSING
    );
}

#[test]
fn sample_values_grow_with_the_sum_of_the_fields_not_their_product() {
    // Twenty integer fields: one-at-a-time sampling needs 20 * 3 + 1 values,
    // where a product would need 3^20.
    let inventory = tank_inventory_with_more_sample_fields((1..20).map(|i| format!("f{i}")));
    let types = content(&plan_of(&tank_model(), &inventory), "src/types.rs");
    let block = types
        .split("impl Sample {")
        .nth(1)
        .and_then(|rest| rest.split("\n        }\n").next())
        .unwrap();
    assert_eq!(block.matches("for &v in &s").count(), 20, "{block}");
    assert!(!block.contains("for f0 in"), "{block}");
    // Every field reaches its largest value, whatever the others hold.
    assert!(block.contains("f19: crate::types::samples_::last(&s19)"));
}

#[test]
fn only_the_root_composite_schedules() {
    let nested = content(&tank_plan(), "src/station/sensing/mod.rs");
    assert!(tank_root().contains("pub struct System {"));
    assert!(!nested.contains("System"));
}

#[test]
fn a_nested_machine_is_scheduled_by_the_root_in_the_flat_network() {
    assert_eq!(
        missing(
            &tank_root(),
            &[
                "fn attempt_filter(&mut self) -> Attempt {",
                "sensing::filter::step(self.filter_state, input)",
            ]
        ),
        NONE_MISSING
    );
}

#[test]
fn output_ports_are_bounded_outboxes_the_environment_empties() {
    assert_eq!(
        missing(
            &tank_root(),
            &[
                "pub fn take_valve_command(&mut self) -> Option<ValveCommandMessage> {",
                "pub const VALVE_COMMAND_CAPACITY: usize = 2;",
            ]
        ),
        NONE_MISSING
    );
}

#[test]
fn a_route_through_several_boundaries_composes_its_translations_in_order() {
    let root = relay_root();
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
}

#[test]
fn two_routes_to_one_receiver_need_room_for_both_before_anything_changes() {
    let root = relay_root();
    assert!(
        root.contains("if self.sink_inbox.len() + to_sink > SINK_CAPACITY {"),
        "{root}"
    );
}

#[test]
fn a_chain_of_forwarded_input_ports_is_applied_in_order() {
    let root = relay_root();
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
const STARTER_DERIVED: &str =
    include_str!("../../../examples/starter/composition/L2RDerived/starter.lean");
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
        (
            starter_model(),
            starter_inventory(),
            STARTER_DERIVED,
            "examples/starter",
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
        .find_map(|l| {
            l.strip_prefix(&format!("-- {key}: "))
                .or_else(|| l.strip_prefix(&format!("// {key}: ")))
        })
        .unwrap_or_else(|| panic!("no `-- {key}:` header line"))
        .to_string()
}

#[test]
fn the_derived_network_names_exactly_what_it_was_derived_from() {
    let record: serde_json::Value = serde_json::from_str(TANK_APPROVAL).expect("approval parses");
    let field = |key: &str| record[key].as_str().expect("string field").to_string();
    let expected = [
        ("model", field("model_digest")),
        ("inventory", field("inventory_digest")),
        ("generator", field("generator")),
        (
            "semantics",
            format!(
                "l2r.async/v1 {}",
                l2r_model::digest::sha256(SEMANTICS.as_bytes())
            ),
        ),
        (
            "scheduler",
            format!(
                "{} {}",
                l2r_plan::PORTS_SCHEDULER_VERSION,
                l2r_model::digest::sha256(SCHEDULER.as_bytes())
            ),
        ),
    ];
    let named = expected
        .clone()
        .map(|(key, _)| (key, header(TANK_DERIVED, key)));
    assert_eq!(named, expected);
}

#[test]
fn the_derived_network_names_its_format() {
    assert!(TANK_DERIVED.starts_with(&format!(
        "-- @generated by L2R ({}).",
        l2r_plan::LEAN_COMPOSITION_FORMAT_V2
    )));
}

#[test]
fn outboxes_are_children_of_the_derived_network() {
    // An outbox restates no behaviour: it is the specification's `outboxStep`,
    // after the three machines.
    assert_eq!(
        missing(
            TANK_DERIVED,
            &[
                "  | ⟨3, _⟩ => L2R.Async1.outboxStep _root_.Tank.Ports.AlarmNotice",
                "def machines : Nat := 3",
            ]
        ),
        NONE_MISSING
    );
}

#[test]
fn derived_routes_compose_the_lean_translations_in_the_order_the_scheduler_applies_them() {
    assert!(TANK_DERIVED.contains(
        "((_root_.Tank.Routes.filterToLevel o).bind _root_.Tank.Routes.levelToController).toList.map (fun i => ⟨⟨1, by decide⟩, i⟩)"
    ));
}

#[test]
fn a_derived_injection_composes_the_lean_port_translations_from_the_root_down() {
    assert!(RELAY_DERIVED.contains(
        "fun m => _root_.Relay.Ports.cellToSource (_root_.Relay.Ports.zoneToCell (_root_.Relay.Ports.plantToZone m))"
    ));
}

#[test]
fn the_committed_conformance_harnesses_are_current() {
    for (model, inventory, rust, lean, path) in [
        (
            tank_model(),
            tank_inventory(),
            include_str!("../../../examples/tank_station/conformance/traces.rs"),
            include_str!("../../../examples/tank_station/conformance/Conformance.lean"),
            "examples/tank_station",
        ),
        (
            relay_model(),
            relay_inventory(),
            include_str!("../../../examples/relay/conformance/traces.rs"),
            include_str!("../../../examples/relay/conformance/Conformance.lean"),
            "examples/relay",
        ),
        (
            starter_model(),
            starter_inventory(),
            include_str!("../../../examples/starter/conformance/traces.rs"),
            include_str!("../../../examples/starter/conformance/Conformance.lean"),
            "examples/starter",
        ),
    ] {
        let validated = validate(&model, &inventory).expect("valid");
        for (side, committed, flag) in [
            (ConformanceSide::Rust, rust, "--rust"),
            (ConformanceSide::Lean, lean, "--lean"),
        ] {
            let fresh = conformance(&validated, side).expect("derives");
            assert!(
                fresh == committed,
                "{path}/conformance is stale; regenerate it with \
                 `l2r conformance {path}/architecture.toml {flag}`"
            );
        }
    }
}

#[test]
fn both_sides_of_the_harness_try_the_same_operations_in_the_same_order() {
    let validated = validate(&tank_model(), &tank_inventory()).expect("valid");
    let rust = conformance(&validated, ConformanceSide::Rust).unwrap();
    let lean = conformance(&validated, ConformanceSide::Lean).unwrap();
    // 1 + 3 + 4 sample messages, step_once, two takes.
    assert!(rust.contains("const OPS: usize = 11;"), "{rust}");
    assert!(lean.contains("  | 10 =>\n    match Ports.take N s (⟨4, by decide⟩ : Fin 5) with"));
    assert!(!lean.contains("  | 11 =>"));
    // The same sample message at the same index, in each language.
    assert!(rust.contains(
        "7 => match system.send_sample(tank_station_scaffold::types::tank::ports::Sample { raw: 255 }) {"
    ));
    assert!(lean.contains(
        "  | 7 =>\n    match Ports.send N s (⟨2, by decide⟩ : Fin N.ports) (_root_.Tank.Ports.Sample.mk 255 : _root_.Tank.Ports.Sample) with"
    ));
    // Bound to what it was derived from, like the derived network.
    for text in [&rust, &lean] {
        assert_eq!(header(text, "model"), header(TANK_DERIVED, "model"));
        assert_eq!(header(text, "scheduler"), header(TANK_DERIVED, "scheduler"));
    }
}

#[test]
fn conformance_is_derived_for_hierarchical_models_only() {
    let v1 = validate(
        &l2r_testing::example_v1_model(),
        &l2r_testing::example_v1_inventory(),
    )
    .unwrap();
    assert!(conformance(&v1, ConformanceSide::Rust).is_err());
}
