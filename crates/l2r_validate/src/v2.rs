//! Rules of model schema `l2r.architecture/v2`: hierarchical composition.
//!
//! A `v2` model has machines (`transition-core/v1`) and composites
//! (`composite/v2`) nested to any depth. Messages still connect siblings only:
//! an edge lives in one composite and joins two things visible there, namely
//! its child machines, the ports of its child composites, and its own ports.
//! Crossing a composite's boundary therefore always goes through a typed port
//! that the architect declared.

use std::collections::{BTreeMap, BTreeSet};

use l2r_model::{
    Component, DeclKind, INVENTORY_SCHEMA_V1, Inventory, MessageEdge, Model, Port, profiles,
};

use crate::data::{self, DataType, TypeUse};
use crate::diagnostic::{Diagnostic, Findings};
use crate::flat::{self, FlatError};
use crate::lean_refs::{self, Refs, port_translation_type, step_type, translation_type};
use crate::names;
use crate::resolved::Validated;
use crate::structure::{self, MAX_MAILBOX_CAPACITY, subject};

/// Identifiers the generated `System` uses as prefixes of its own methods.
const RESERVED_IDS: &[&str] = &["attempt", "send", "take"];

/// Validates a canonical `v2` model against its inventory.
pub fn validate(model: Model, inventory: Inventory) -> Result<Validated, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    structure::check_common(&model, &mut findings);
    check_components(&model, &mut findings);
    check_ports(&model, &mut findings);
    check_edges(&model, &mut findings);
    check_generated_names(&model, &mut findings);
    lean_refs::check_ownership(&model, &mut findings);

    let mut types = BTreeMap::new();
    if inventory.schema != INVENTORY_SCHEMA_V1 {
        findings.error(
            "L2R-L001",
            "lean inventory",
            format!(
                "model schema v2 needs an inventory with constructor fields: expected `{INVENTORY_SCHEMA_V1}`, found `{}` (export with `--v1`)",
                inventory.schema
            ),
        );
    } else {
        types = check_lean(&model, &inventory, &mut findings);
    }

    // The flat network is defined only for a structurally valid model.
    let flat = if findings.is_empty() {
        match flat::flatten(&model) {
            Ok(flat) => Some(flat),
            Err(errors) => {
                for error in errors {
                    match error {
                        FlatError::SelfRoute { machine, via } => findings.error(
                            "L2R-M008",
                            format!("component `{machine}`"),
                            format!(
                                "an output can return to its own mailbox through [{}]; express this inside the transition function",
                                via.join(", ")
                            ),
                        ),
                        FlatError::TooManyRoutes { machine } => findings.error(
                            "L2R-M009",
                            format!("component `{machine}`"),
                            format!(
                                "one output reaches more than {} receivers through the approved edges and ports",
                                flat::MAX_ROUTES_PER_MACHINE
                            ),
                        ),
                    }
                }
                None
            }
        }
    } else {
        None
    };

    match flat {
        Some(flat) if findings.is_empty() => Ok(Validated::new_v2(model, inventory, types, flat)),
        _ => Err(findings.into_sorted()),
    }
}

fn is_machine(component: &Component) -> bool {
    component.profile == profiles::TRANSITION_CORE_V1
}

fn is_composite(component: &Component) -> bool {
    component.profile == profiles::COMPOSITE_V2
}

fn check_components(model: &Model, findings: &mut Findings) {
    for component in &model.components {
        let subject = subject(component);
        let has_children = model
            .components
            .iter()
            .any(|c| c.parent.as_deref() == Some(&component.id));
        if !is_machine(component) && !is_composite(component) {
            findings.error(
                "L2R-S019",
                &subject,
                format!(
                    "profile `{}` cannot be used in a v2 model; use `{}` for machines and `{}` for composites",
                    component.profile,
                    profiles::TRANSITION_CORE_V1,
                    profiles::COMPOSITE_V2
                ),
            );
            continue;
        }
        if RESERVED_IDS.contains(&component.id.as_str()) {
            findings.error(
                "L2R-S010",
                &subject,
                format!("the identifier `{}` is reserved", component.id),
            );
        }
        if is_machine(component) {
            if component.behavior.is_none() {
                findings.error(
                    "L2R-S008",
                    &subject,
                    "a machine requires a [component.behavior] table",
                );
            }
            if has_children {
                findings.error(
                    "L2R-S008",
                    &subject,
                    "a machine cannot contain other components",
                );
            }
            match component.mailbox_capacity {
                None => findings.error(
                    "L2R-S017",
                    &subject,
                    "a machine must state `mailbox_capacity`",
                ),
                Some(0) => findings.error(
                    "L2R-S017",
                    &subject,
                    "`mailbox_capacity` must be at least 1",
                ),
                Some(n) if n > MAX_MAILBOX_CAPACITY => findings.error(
                    "L2R-S017",
                    &subject,
                    format!(
                        "`mailbox_capacity` {n} exceeds the supported maximum {MAX_MAILBOX_CAPACITY}"
                    ),
                ),
                Some(_) => {}
            }
        } else {
            if component.behavior.is_some() {
                findings.error(
                    "L2R-S008",
                    &subject,
                    "a composite owns no behavioral state; remove [component.behavior]",
                );
            }
            if !has_children {
                findings.error(
                    "L2R-S008",
                    &subject,
                    "a composite must contain at least one component",
                );
            }
            if component.mailbox_capacity.is_some() {
                findings.error(
                    "L2R-S018",
                    &subject,
                    "`mailbox_capacity` applies only to machines",
                );
            }
            if component.parent.is_none() && component.rust_module == "types" {
                findings.error(
                    "L2R-S010",
                    &subject,
                    "the root module cannot be `types`: the generated crate has a module of that name",
                );
            }
        }
    }
    if let Some(root) = model.components.iter().find(|c| c.parent.is_none())
        && is_machine(root)
    {
        findings.error(
            "L2R-S016",
            subject(root),
            "the root of a v2 model must be a composite/v2: it generates the scheduler and owns the system's ports",
        );
    }
}

fn port_subject(port: &Port) -> String {
    format!("port `{}`", port.id)
}

fn root_id(model: &Model) -> Option<&str> {
    model
        .components
        .iter()
        .find(|c| c.parent.is_none())
        .map(|c| c.id.as_str())
}

fn check_ports(model: &Model, findings: &mut Findings) {
    let mut ids = BTreeSet::new();
    let root = root_id(model);
    for port in &model.ports {
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
        let composite = model.component(&port.composite).filter(|c| is_composite(c));
        if composite.is_none() {
            findings.error(
                "L2R-P002",
                &subject,
                format!("`{}` is not a composite/v2 component", port.composite),
            );
        }
        let on_root = root == Some(port.composite.as_str());
        match port.direction.as_deref() {
            Some("in") => {
                if port.capacity.is_some() {
                    findings.error(
                        "L2R-P008",
                        &subject,
                        "`capacity` applies only to `out` ports of the root composite; an `in` port has no buffer of its own",
                    );
                }
                check_in_target(model, port, &subject, findings);
            }
            Some("out") => {
                if port.to.is_some() || port.translation.is_some() {
                    findings.error(
                        "L2R-P009",
                        &subject,
                        "an `out` port has no `to` or `translation`; message edges inside its composite feed it",
                    );
                }
                match (on_root, port.capacity) {
                    (true, None) => findings.error(
                        "L2R-P008",
                        &subject,
                        "an `out` port of the root composite must state the `capacity` of its outbox",
                    ),
                    (true, Some(n)) if n == 0 || n > MAX_MAILBOX_CAPACITY => findings.error(
                        "L2R-P008",
                        &subject,
                        format!("`capacity` must be between 1 and {MAX_MAILBOX_CAPACITY}"),
                    ),
                    (false, Some(_)) => findings.error(
                        "L2R-P008",
                        &subject,
                        "only the root composite's `out` ports have an outbox; this port forwards to the edges of its parent",
                    ),
                    _ => {}
                }
                if !on_root
                    && composite.is_some()
                    && !model.messages.iter().any(|e| e.from == port.id)
                {
                    findings.error(
                        "L2R-M007",
                        &subject,
                        "has no outgoing message edge in the parent composite, so every message reaching it would be discarded",
                    );
                }
            }
            _ => findings.error(
                "L2R-P007",
                &subject,
                "a v2 port must state `direction = \"in\"` or `direction = \"out\"`",
            ),
        }
    }
    if let Some(root) = root
        && !model
            .ports
            .iter()
            .any(|p| p.composite == root && p.direction.as_deref() == Some("in"))
    {
        findings.error(
            "L2R-P004",
            format!("component `{root}`"),
            "the root composite has no `in` port, so the system can receive nothing from outside",
        );
    }
}

/// An `in` port feeds one child machine or one `in` port of a child composite.
fn check_in_target(model: &Model, port: &Port, subject: &str, findings: &mut Findings) {
    let (Some(target), Some(_)) = (port.to.as_deref(), port.translation.as_deref()) else {
        findings.error(
            "L2R-P003",
            subject,
            "an `in` port must state `to` (what it feeds) and `translation`",
        );
        return;
    };
    let child_of_composite = |c: &Component| c.parent.as_deref() == Some(port.composite.as_str());
    let ok = match model.component(target) {
        Some(machine) => is_machine(machine) && child_of_composite(machine),
        None => model.ports.iter().any(|p| {
            p.id == target
                && p.direction.as_deref() == Some("in")
                && model
                    .component(&p.composite)
                    .is_some_and(|c| is_composite(c) && child_of_composite(c))
        }),
    };
    if !ok {
        findings.error(
            "L2R-P003",
            subject,
            format!(
                "`{target}` is neither a machine child of `{}` nor an `in` port of one of its child composites",
                port.composite
            ),
        );
    }
}

/// What an edge endpoint names.
enum Endpoint<'a> {
    Machine(&'a Component),
    Port(&'a Port),
}

fn endpoint<'a>(model: &'a Model, id: &str) -> Option<Endpoint<'a>> {
    if let Some(component) = model.component(id) {
        return is_machine(component).then_some(Endpoint::Machine(component));
    }
    model.ports.iter().find(|p| p.id == id).map(Endpoint::Port)
}

fn parent_of<'a>(model: &'a Model, id: &str) -> Option<&'a str> {
    model.component(id).and_then(|c| c.parent.as_deref())
}

fn check_edges(model: &Model, findings: &mut Findings) {
    let mut ids = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    for edge in &model.messages {
        let subject = format!("message `{}`", edge.id);
        if !names::is_identifier(&edge.id) || names::is_rust_keyword(&edge.id) {
            findings.error(
                "L2R-S002",
                &subject,
                "message id must be a lower-snake-case identifier usable as a Rust function name",
            );
        }
        if !ids.insert(edge.id.as_str()) {
            findings.error("L2R-M001", &subject, "duplicate message id");
        }
        if !pairs.insert((edge.from.as_str(), edge.to.as_str())) {
            findings.error(
                "L2R-M006",
                &subject,
                format!(
                    "a second edge from `{}` to `{}` would make delivery ambiguous",
                    edge.from, edge.to
                ),
            );
        }
        check_edge_endpoints(model, edge, &subject, findings);
    }
    for machine in model.components.iter().filter(|c| is_machine(c)) {
        if !model.messages.iter().any(|e| e.from == machine.id) {
            findings.error(
                "L2R-M007",
                subject(machine),
                "has no outgoing message edge, so every output it emits would be discarded; route its outputs to a sibling or to an `out` port",
            );
        }
    }
}

fn check_edge_endpoints(model: &Model, edge: &MessageEdge, subject: &str, findings: &mut Findings) {
    if edge.from == edge.to {
        findings.error(
            "L2R-M004",
            subject,
            "an edge cannot connect something to itself",
        );
        return;
    }
    // The composite the edge lives in: where its source is visible.
    let scope = match endpoint(model, &edge.from) {
        None => {
            let code = if model.component(&edge.from).is_some() {
                "L2R-M003"
            } else {
                "L2R-M002"
            };
            findings.error(
                code,
                subject,
                format!(
                    "from `{}` is not a machine or an `out` port of a child composite",
                    edge.from
                ),
            );
            None
        }
        Some(Endpoint::Machine(machine)) => machine.parent.as_deref(),
        Some(Endpoint::Port(port)) => {
            if port.direction.as_deref() != Some("out") {
                findings.error(
                    "L2R-M003",
                    subject,
                    format!(
                        "from `{}` is an `in` port; an `in` port forwards through its own `to`, not through edges",
                        edge.from
                    ),
                );
                None
            } else {
                match parent_of(model, &port.composite) {
                    Some(parent) => Some(parent),
                    None => {
                        findings.error(
                            "L2R-M003",
                            subject,
                            format!(
                                "from `{}` is an `out` port of the root composite; its messages leave the system",
                                edge.from
                            ),
                        );
                        None
                    }
                }
            }
        }
    };
    let target_scope = match endpoint(model, &edge.to) {
        None => {
            let code = if model.component(&edge.to).is_some() {
                "L2R-M003"
            } else {
                "L2R-M002"
            };
            findings.error(
                code,
                subject,
                format!("to `{}` is not a machine or a port", edge.to),
            );
            return;
        }
        Some(Endpoint::Machine(machine)) => machine.parent.as_deref(),
        Some(Endpoint::Port(port)) if port.direction.as_deref() == Some("out") => {
            Some(port.composite.as_str())
        }
        Some(Endpoint::Port(port)) => parent_of(model, &port.composite),
    };
    if let Some(scope) = scope
        && target_scope != Some(scope)
    {
        findings.error(
            "L2R-M005",
            subject,
            format!(
                "`{}` and `{}` are not both visible in composite `{scope}`: an edge joins that composite's child machines, the ports of its child composites, and its own `out` ports",
                edge.from, edge.to
            ),
        );
    }
}

/// Names the root's generated code derives from identifiers must be distinct.
fn check_generated_names(model: &Model, findings: &mut Findings) {
    let root = root_id(model);
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let machines = model
        .components
        .iter()
        .filter(|c| is_machine(c))
        .map(|c| (subject(c), c.id.as_str()));
    let ports = model
        .ports
        .iter()
        .filter(|p| root == Some(p.composite.as_str()))
        .map(|p| (port_subject(p), p.id.as_str()));
    for (subject, id) in machines.chain(ports) {
        let type_name = names::upper_camel(id).unwrap_or_default();
        if let Some(other) = seen.insert(type_name.clone(), subject.clone()) {
            findings.error(
                "L2R-S020",
                subject,
                format!("generates the same Rust type prefix `{type_name}` as {other}"),
            );
        }
    }
    // In `routes.rs` an edge is the function `<id>` and an input port `port_<id>`.
    for edge in &model.messages {
        if let Some(port) = edge.id.strip_prefix("port_")
            && model.ports.iter().any(|p| p.id == port)
        {
            findings.error(
                "L2R-S020",
                format!("message `{}`", edge.id),
                format!("generates the same function name as port `{port}`"),
            );
        }
    }
}

/// Resolves every Lean reference; returns the data types used.
fn check_lean(
    model: &Model,
    inventory: &Inventory,
    findings: &mut Findings,
) -> BTreeMap<String, DataType> {
    let mut uses = Vec::new();
    for component in &model.components {
        let Some(behavior) = &component.behavior else {
            continue;
        };
        for (role, name) in [
            ("owned_state", &behavior.owned_state),
            ("input", &behavior.input),
            ("output", &behavior.output),
        ] {
            uses.push(TypeUse {
                subject: subject(component),
                role,
                lean_name: name,
            });
        }
        let refs = Refs {
            subject: subject(component),
            inventory,
        };
        refs.expect_typed(
            &behavior.init,
            "init",
            DeclKind::Definition,
            &behavior.owned_state,
            findings,
        );
        refs.expect_typed(
            &behavior.step,
            "step",
            DeclKind::Definition,
            &step_type(&behavior.owned_state, &behavior.input, &behavior.output),
            findings,
        );
        for invariant in &behavior.invariants {
            refs.expect_decl(invariant, "invariant", DeclKind::Theorem, findings);
        }
    }
    let input_type = |id: &str| -> Option<&str> {
        match endpoint(model, id)? {
            Endpoint::Machine(machine) => machine.behavior.as_ref().map(|b| b.input.as_str()),
            Endpoint::Port(port) => Some(port.message.as_str()),
        }
    };
    let output_type = |id: &str| -> Option<&str> {
        match endpoint(model, id)? {
            Endpoint::Machine(machine) => machine.behavior.as_ref().map(|b| b.output.as_str()),
            Endpoint::Port(port) => Some(port.message.as_str()),
        }
    };
    for port in &model.ports {
        uses.push(TypeUse {
            subject: port_subject(port),
            role: "message",
            lean_name: &port.message,
        });
        if let (Some(target), Some(translation)) = (port.to.as_deref(), port.translation.as_deref())
            && let Some(target_type) = input_type(target)
        {
            Refs {
                subject: port_subject(port),
                inventory,
            }
            .expect_typed(
                translation,
                "translation",
                DeclKind::Definition,
                &port_translation_type(&port.message, target_type),
                findings,
            );
        }
    }
    for edge in &model.messages {
        if let (Some(from), Some(to)) = (output_type(&edge.from), input_type(&edge.to)) {
            Refs {
                subject: format!("message `{}`", edge.id),
                inventory,
            }
            .expect_typed(
                &edge.translation,
                "translation",
                DeclKind::Definition,
                &translation_type(from, to),
                findings,
            );
        }
    }
    data::resolve(&uses, inventory, findings)
}
