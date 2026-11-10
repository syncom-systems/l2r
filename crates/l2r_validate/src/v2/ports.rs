//! Port rules of schema `v2`: a port belongs to a composite and has a
//! direction. An `in` port feeds one child of its composite; an `out` port is
//! fed by that composite's edges and, on the root, is a bounded outbox.

use std::collections::BTreeSet;

use l2r_model::{Component, Model, Port};

use super::{RESERVED_IDS, has_outgoing_edge, is_composite, is_machine, port_subject, root_id};
use crate::diagnostic::Findings;
use crate::names;
use crate::structure::MAX_MAILBOX_CAPACITY;

fn is_in(port: &Port) -> bool {
    port.direction.as_deref() == Some("in")
}

pub(super) fn check(model: &Model, findings: &mut Findings) {
    let mut ids = BTreeSet::new();
    for port in &model.ports {
        check_port_identity(model, port, &mut ids, findings);
        check_port_direction(model, port, findings);
    }
    check_root_can_receive(model, findings);
}

/// A port's identifier is usable, unique (not among `ids`, those of the ports
/// before it), distinct from every component's, and its composite exists.
fn check_port_identity<'a>(
    model: &Model,
    port: &'a Port,
    ids: &mut BTreeSet<&'a str>,
    findings: &mut Findings,
) {
    let subject = port_subject(port);
    if !names::is_identifier(&port.id) || names::is_rust_keyword(&port.id) {
        findings.error(
            "L2R-S002",
            &subject,
            "port id must be a lower-snake-case identifier usable in a Rust function name",
        );
    }
    if RESERVED_IDS.contains(&port.id.as_str()) {
        findings.error(
            "L2R-S010",
            &subject,
            format!("the identifier `{}` is reserved", port.id),
        );
    }
    if !ids.insert(port.id.as_str()) {
        findings.error("L2R-P001", &subject, "duplicate port id");
    }
    if model.component(&port.id).is_some() {
        findings.error(
            "L2R-P006",
            &subject,
            "a port and a component cannot share an identifier: message edges name both",
        );
    }
    if !is_composite_id(model, &port.composite) {
        findings.error(
            "L2R-P002",
            &subject,
            format!("`{}` is not a composite/v2 component", port.composite),
        );
    }
}

fn is_composite_id(model: &Model, id: &str) -> bool {
    model.component(id).is_some_and(is_composite)
}

/// A port states its direction and has the fields of that direction.
fn check_port_direction(model: &Model, port: &Port, findings: &mut Findings) {
    match port.direction.as_deref() {
        Some("in") => check_in_port(model, port, findings),
        Some("out") => check_out_port(model, port, findings),
        _ => findings.error(
            "L2R-P007",
            port_subject(port),
            "a v2 port must state `direction = \"in\"` or `direction = \"out\"`",
        ),
    }
}

/// Without an `in` port on the root, nothing enters the system.
fn check_root_can_receive(model: &Model, findings: &mut Findings) {
    let Some(root) = root_id(model) else {
        return;
    };
    let receives = model.ports.iter().any(|p| p.composite == root && is_in(p));
    if !receives {
        findings.error(
            "L2R-P004",
            format!("component `{root}`"),
            "the root composite has no `in` port, so the system can receive nothing from outside",
        );
    }
}

fn check_in_port(model: &Model, port: &Port, findings: &mut Findings) {
    if port.capacity.is_some() {
        findings.error(
            "L2R-P008",
            port_subject(port),
            "`capacity` applies only to `out` ports of the root composite; an `in` port has no buffer of its own",
        );
    }
    check_in_target(model, port, findings);
}

fn check_out_port(model: &Model, port: &Port, findings: &mut Findings) {
    let subject = port_subject(port);
    if port.to.is_some() || port.translation.is_some() {
        findings.error(
            "L2R-P009",
            &subject,
            "an `out` port has no `to` or `translation`; message edges inside its composite feed it",
        );
    }
    let on_root = root_id(model) == Some(port.composite.as_str());
    if let Some(problem) = outbox_capacity_problem(on_root, port.capacity) {
        findings.error("L2R-P008", &subject, problem);
    }
    if !on_root {
        check_nested_out_port_is_read(model, port, findings);
    }
}

/// Only the root's `out` ports have an outbox, and each states its bound.
fn outbox_capacity_problem(on_root: bool, capacity: Option<u32>) -> Option<String> {
    match (on_root, capacity) {
        (true, None) => {
            Some("an `out` port of the root composite must state the `capacity` of its outbox".to_string())
        }
        (true, Some(n)) if !(1..=MAX_MAILBOX_CAPACITY).contains(&n) => Some(format!(
            "`capacity` must be between 1 and {MAX_MAILBOX_CAPACITY}"
        )),
        (false, Some(_)) => Some(
            "only the root composite's `out` ports have an outbox; this port forwards to the edges of its parent"
                .to_string(),
        ),
        _ => None,
    }
}

/// An `out` port below the root forwards to the edges of its parent composite.
fn check_nested_out_port_is_read(model: &Model, port: &Port, findings: &mut Findings) {
    // A port whose composite does not exist is rejected as such; nothing could read it.
    if !is_composite_id(model, &port.composite) {
        return;
    }
    if !has_outgoing_edge(model, &port.id) {
        findings.error(
            "L2R-M007",
            port_subject(port),
            "has no outgoing message edge in the parent composite, so every message reaching it would be discarded",
        );
    }
}

fn is_child_of(component: &Component, composite: &str) -> bool {
    component.parent.as_deref() == Some(composite)
}

/// Whether `port` is an `in` port of a composite directly inside `composite`.
fn is_in_port_of_child(model: &Model, port: &Port, composite: &str) -> bool {
    let owner_is_child = model
        .component(&port.composite)
        .is_some_and(|c| is_composite(c) && is_child_of(c, composite));
    is_in(port) && owner_is_child
}

/// Whether an `in` port of `composite` may feed `target`: one child machine or
/// one `in` port of a child composite.
fn is_in_target(model: &Model, target: &str, composite: &str) -> bool {
    match model.component(target) {
        Some(component) => is_machine(component) && is_child_of(component, composite),
        None => model
            .ports
            .iter()
            .any(|p| p.id == target && is_in_port_of_child(model, p, composite)),
    }
}

/// An `in` port feeds one child machine or one `in` port of a child composite.
fn check_in_target(model: &Model, port: &Port, findings: &mut Findings) {
    let (Some(target), Some(_)) = (port.to.as_deref(), port.translation.as_deref()) else {
        findings.error(
            "L2R-P003",
            port_subject(port),
            "an `in` port must state `to` (what it feeds) and `translation`",
        );
        return;
    };
    if !is_in_target(model, target, &port.composite) {
        findings.error(
            "L2R-P003",
            port_subject(port),
            format!(
                "`{target}` is neither a machine child of `{}` nor an `in` port of one of its child composites",
                port.composite
            ),
        );
    }
}
