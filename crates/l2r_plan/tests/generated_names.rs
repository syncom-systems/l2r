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
    assert!(types.contains("System") && !route_functions.is_empty());
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

#[test]
fn composite_v0_emits_exactly_the_declared_names() {
    let validated = validate(&example_model(), &example_inventory()).expect("valid");
    let names = validated.composite_v0_names("crossing").expect("checked");
    let found = emitted(&planned(&validated), "src/crossing");
    assert_eq!(found.fields, set(names.fields()));
    assert_eq!(found.methods, set(names.methods()));
    assert_eq!(found.types, set(names.types()));
    assert_eq!(found.route_functions, set(names.route_functions()));
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
