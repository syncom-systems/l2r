//! The names `l2r_model::rust_names` declares for `composite/v0` and
//! `composite/v1` are exactly the names the planner emits: every field and
//! method of `System`, every module-level type and every function of
//! `routes.rs`, read back from the planned files. Validation rejects conflicts
//! among the declared names, so this test is what ties that check to the code.

use std::collections::BTreeSet;

use l2r_plan::{GeneratorIdentity, Plan, plan};
use l2r_testing::{example_inventory, example_model, example_v1_inventory, example_v1_model};
use l2r_validate::{Validated, validate};

const RING: &str = include_str!("fixtures/ring.toml");
const RING_INVENTORY: &str = include_str!("fixtures/ring-inventory.json");

/// The lines of `text` inside the block that starts with the line `opening`.
fn block<'a>(text: &'a str, opening: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|l| *l != opening)
        .skip(1)
        .take_while(|l| *l != "}")
        .collect()
}

fn names_after<'a>(lines: impl Iterator<Item = &'a str>, prefixes: &[&str]) -> BTreeSet<String> {
    lines
        .filter_map(|l| prefixes.iter().find_map(|p| l.strip_prefix(p)))
        .map(|rest| {
            rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

struct Emitted {
    fields: BTreeSet<String>,
    methods: BTreeSet<String>,
    types: BTreeSet<String>,
    route_functions: BTreeSet<String>,
}

fn emitted(plan: &Plan, dir: &str) -> Emitted {
    let contract = &plan
        .artifact(&format!("{dir}/mod.rs"))
        .expect("planned")
        .content;
    let routes = plan
        .artifact(&format!("{dir}/routes.rs"))
        .map(|a| a.content.as_str())
        .unwrap_or_default();
    let fields = names_after(
        block(contract, "pub struct System {").into_iter(),
        &["    "],
    );
    let methods = names_after(
        block(contract, "impl System {").into_iter(),
        &["    pub fn ", "    fn ", "    const "],
    );
    let types = names_after(
        contract.lines(),
        &[
            "pub struct ",
            "pub enum ",
            "enum ",
            "use std::collections::",
        ],
    );
    let route_functions = names_after(routes.lines(), &["pub(super) fn "]);
    // The reading is not vacuous: each namespace has what every composite has.
    assert!(fields.contains("next") && methods.contains("step_once"));
    // `routes.rs` exists only when the composite has edges; where it does,
    // its functions are compared like everything else.
    assert!(types.contains("System") && (routes.is_empty() || !route_functions.is_empty()));
    Emitted {
        fields,
        methods,
        types,
        route_functions,
    }
}

fn set(names: &[String]) -> BTreeSet<String> {
    names.iter().cloned().collect()
}

fn planned(validated: &Validated) -> Plan {
    plan(validated, &GeneratorIdentity::current()).expect("plans")
}

/// The v0 example with its two machines moved into a nested `composite/v0`
/// `inner`, so a composite child (a field with no mailbox) is read back too.
fn nested_v0() -> l2r_model::Model {
    let mut model = example_model();
    let mut inner = model
        .components
        .iter()
        .find(|c| c.id == "crossing")
        .expect("root")
        .clone();
    for c in &mut model.components {
        if c.parent.as_deref() == Some("crossing") {
            c.parent = Some("inner".into());
        }
    }
    inner.id = "inner".into();
    inner.name = "Inner".into();
    inner.parent = Some("crossing".into());
    inner.rust_module = "inner".into();
    model.components.push(inner);
    model
}

#[test]
fn composite_v0_emits_exactly_the_declared_names() {
    for (model, composites) in [
        (example_model(), vec!["src/crossing"]),
        (nested_v0(), vec!["src/crossing", "src/crossing/inner"]),
    ] {
        let validated = validate(&model, &example_inventory()).expect("valid");
        let plan = planned(&validated);
        for dir in composites {
            let id = dir.rsplit('/').next().expect("id");
            let names = validated.composite_v0_names(id).expect("checked");
            let found = emitted(&plan, dir);
            assert_eq!(found.fields, set(names.fields()), "{dir}");
            assert_eq!(found.methods, set(names.methods()), "{dir}");
            assert_eq!(found.types, set(names.types()), "{dir}");
            assert_eq!(found.route_functions, set(names.route_functions()), "{dir}");
            let contract = &plan
                .artifact(&format!("{dir}/mod.rs"))
                .expect("planned")
                .content;
            let variants: BTreeSet<String> = block(contract, "pub enum Envelope {")
                .into_iter()
                .filter(|l| !l.trim_start().starts_with("///"))
                .filter_map(|l| l.trim_start().split('(').next().map(str::to_string))
                .collect();
            let declared: BTreeSet<String> =
                names.variants().into_iter().map(str::to_string).collect();
            assert!(!variants.is_empty());
            assert_eq!(variants, declared, "{dir}");
        }
    }
}

#[test]
fn composite_v1_emits_exactly_the_declared_names() {
    let ring_model = l2r_model::load::parse_model(RING).expect("ring parses");
    let ring_inventory = l2r_model::load::parse_inventory(RING_INVENTORY).expect("parses");
    for (model, inventory, root) in [
        (example_v1_model(), example_v1_inventory(), "crossing"),
        (ring_model, ring_inventory, "ring"),
    ] {
        let validated = validate(&model, &inventory).expect("valid");
        let names = validated.composite_v1_names(root).expect("checked");
        let found = emitted(&planned(&validated), &format!("src/{root}"));
        assert_eq!(found.fields, set(names.fields()), "{root}");
        assert_eq!(found.methods, set(names.methods()), "{root}");
        assert_eq!(found.types, set(names.types()), "{root}");
        assert_eq!(
            found.route_functions,
            set(names.route_functions()),
            "{root}"
        );
    }
}

/// The `System` of a `composite/v2` root declares exactly the members
/// `rust_names::composite_v2` lists: its own (`SYSTEM_MEMBERS`) and those of
/// every machine and root port. S020 checks conflicts among these lists, so
/// this ties that rule to the code.
#[test]
fn composite_v2_emits_exactly_the_declared_members() {
    use l2r_model::rust_names::composite_v2::{InPort, Machine, OutPort, SYSTEM_MEMBERS};
    use l2r_testing::{relay_inventory, relay_model, starter_inventory, starter_model};
    use l2r_testing::{tank_inventory, tank_model};
    for (model, inventory) in [
        (relay_model(), relay_inventory()),
        (tank_model(), tank_inventory()),
        (starter_model(), starter_inventory()),
    ] {
        let validated = validate(&model, &inventory).expect("valid");
        let flat = validated.flat().expect("v2");
        let mut declared: BTreeSet<String> = SYSTEM_MEMBERS.iter().map(|m| m.to_string()).collect();
        declared.extend(flat.leaves.iter().flat_map(|l| Machine(&l.id).members()));
        declared.extend(flat.inputs.iter().flat_map(|i| InPort(&i.port).members()));
        declared.extend(flat.sinks.iter().flat_map(|s| OutPort(&s.port).members()));
        let plan = planned(&validated);
        let contract = plan
            .artifacts
            .iter()
            .map(|a| a.content.as_str())
            .find(|c| c.contains("\npub struct System {\n"))
            .expect("the root declares System");
        let mut emitted = names_after(
            block(contract, "pub struct System {").into_iter(),
            &["    "],
        );
        emitted.extend(names_after(
            block(contract, "impl System {").into_iter(),
            &["    pub fn ", "    fn "],
        ));
        assert!(emitted.contains("next") && emitted.contains("step_once"));
        assert_eq!(emitted, declared, "{}", model.system.id);
    }
}
