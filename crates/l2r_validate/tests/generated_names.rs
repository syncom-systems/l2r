//! Generated names of `composite/v0` and `composite/v1` (S021, P005): no two
//! generated names meet, none hides a standard name the code uses, and distinct
//! Lean types stay distinct Rust types. Each case is a minimal change to an
//! example model, with the exact diagnostic.

use l2r_model::{Component, Inventory, Model};
use l2r_testing::{example_inventory, example_model, example_v1_inventory, example_v1_model};
use l2r_validate::validate;

fn component<'a>(model: &'a mut Model, id: &str) -> &'a mut Component {
    model
        .components
        .iter_mut()
        .find(|c| c.id == id)
        .expect("component exists")
}

fn diagnostics(model: &Model, inventory: &Inventory) -> Vec<(String, String, String)> {
    match validate(model, inventory) {
        Ok(_) => vec![],
        Err(found) => found
            .into_iter()
            .map(|d| (d.code.to_string(), d.subject, d.message))
            .collect(),
    }
}

fn finding(code: &str, subject: &str, message: &str) -> (String, String, String) {
    (code.into(), subject.into(), message.into())
}

#[test]
fn a_child_module_named_like_a_generated_member_is_rejected() {
    // The reproduced defect: a module `next` gave `System` two fields `next`
    // (rustc E0124), in v0 and in v1 alike.
    let next = "generates the `System` field `next`, which the profile also generates";
    let latch_inbox = "generates the `System` field `request_latch_inbox`, which component `request_latch` also generates";
    for (model, inventory, module, message) in [
        (example_model(), example_inventory(), "next", next),
        (
            example_model(),
            example_inventory(),
            "request_latch_inbox",
            latch_inbox,
        ),
        (example_v1_model(), example_v1_inventory(), "next", next),
    ] {
        let mut model = model;
        component(&mut model, "signal").rust_module = module.into();
        assert_eq!(
            diagnostics(&model, &inventory),
            vec![finding("L2R-S021", "component `signal`", message)],
            "{} with module `{module}`",
            model.schema
        );
    }
}

/// The v1 example inventory with one more port message type, `name`, a copy of
/// `CrossingV1.Ports.TimerEvent`, and its translation `CrossingV1.Other.toSignal`.
fn v1_inventory_with(name: &str) -> Inventory {
    let mut inventory = example_v1_inventory();
    let find = |n: &str| {
        inventory
            .declarations
            .iter()
            .find(|d| d.name == n)
            .cloned()
            .expect("declared")
    };
    let mut ty = find("CrossingV1.Ports.TimerEvent");
    ty.name = name.into();
    for ctor in &mut ty.constructors {
        ctor.name = ctor.name.replace("CrossingV1.Ports.TimerEvent", name);
    }
    let mut translation = find("CrossingV1.Ports.timerToSignal");
    translation.name = "CrossingV1.Other.toSignal".into();
    translation.type_ = translation
        .type_
        .replace("CrossingV1.Ports.TimerEvent", name);
    inventory.declarations.extend([ty, translation]);
    inventory
}

fn v1_timer_carrying(name: &str) -> Model {
    let mut model = example_v1_model();
    let timer = model.ports.iter_mut().find(|p| p.id == "timer").unwrap();
    timer.message = name.into();
    timer.translation = Some("CrossingV1.Other.toSignal".into());
    model
}

#[test]
fn distinct_lean_types_that_would_share_a_rust_type_are_rejected() {
    // The reproduced defect: `CrossingV1.Other.ButtonEvent` became the enum
    // mirroring `CrossingV1.Ports.ButtonEvent`.
    let name = "CrossingV1.Other.ButtonEvent";
    assert_eq!(
        diagnostics(&v1_timer_carrying(name), &v1_inventory_with(name)),
        vec![finding(
            "L2R-P005",
            "port `timer`",
            "generates the type `ButtonEvent`, which Lean type `CrossingV1.Ports.ButtonEvent` also generates"
        )]
    );
}

#[test]
fn a_port_type_with_no_usable_or_free_rust_name_is_rejected() {
    for (name, message) in [
        (
            "CrossingV1.Other.Timer'",
            "generates the type `Timer'`, which is not a usable Rust name",
        ),
        (
            "CrossingV1.Other.Self",
            "generates the type `Self`, which is not a usable Rust name",
        ),
        (
            "CrossingV1.Other.System",
            "generates the type `System`, which the profile also generates",
        ),
        (
            "CrossingV1.Other.Option",
            "generates the type `Option`, which would hide the standard `Option` the generated code uses",
        ),
        (
            "CrossingV1.Other.Result",
            "generates the type `Result`, which would hide the standard `Result` the generated code uses",
        ),
        (
            "CrossingV1.Other.Default",
            "generates the type `Default`, which would hide the standard `Default` the generated code uses",
        ),
        (
            "CrossingV1.Other.SignalInbox",
            "generates the type `SignalInbox`, which component `signal` also generates",
        ),
    ] {
        assert_eq!(
            diagnostics(&v1_timer_carrying(name), &v1_inventory_with(name)),
            vec![finding("L2R-P005", "port `timer`", message)],
            "{name}"
        );
    }
}

#[test]
fn an_edge_named_like_a_port_function_is_rejected() {
    let mut model = example_v1_model();
    let edge = model.messages.iter_mut().next().unwrap();
    let old = edge.id.clone();
    edge.id = "port_timer".into();
    let found = diagnostics(&model, &example_v1_inventory());
    assert_eq!(
        found,
        vec![finding(
            "L2R-S021",
            "message `port_timer`",
            "generates the `routes.rs` function `port_timer`, which port `timer` also generates"
        )],
        "renamed {old}"
    );
}

#[test]
fn a_v0_child_module_named_std_is_rejected() {
    let mut model = example_model();
    component(&mut model, "signal").rust_module = "std".into();
    assert_eq!(
        diagnostics(&model, &example_inventory()),
        vec![finding(
            "L2R-S021",
            "component `signal`",
            "generates the module `std`, which would hide the standard `std` the generated code uses"
        )]
    );
}

#[test]
fn a_mistake_other_rules_report_is_not_reported_again_as_a_name_conflict() {
    // Two siblings with one module are S009; the fields they would share are
    // the same mistake, not a second one.
    let mut model = example_model();
    component(&mut model, "signal").rust_module = "request_latch".into();
    let codes: Vec<String> = diagnostics(&model, &example_inventory())
        .into_iter()
        .map(|(code, _, _)| code)
        .collect();
    assert_eq!(codes, vec!["L2R-S009".to_string()]);
}
