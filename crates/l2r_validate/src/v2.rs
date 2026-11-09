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
            Ok(flat) => {
                check_reachable(&model, &flat, &mut findings);
                Some(flat)
            }
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
        check_port_identity(model, port, &subject, &mut ids, findings);
        let on_root = root == Some(port.composite.as_str());
        match port.direction.as_deref() {
            Some("in") => check_in_port(model, port, &subject, findings),
            Some("out") => check_out_port(model, port, &subject, on_root, findings),
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

/// A port's identifier is usable, unique, distinct from every component's, and
/// its composite exists.
fn check_port_identity<'a>(
    model: &Model,
    port: &'a Port,
    subject: &str,
    ids: &mut BTreeSet<&'a str>,
    findings: &mut Findings,
) {
    if !names::is_identifier(&port.id) || names::is_rust_keyword(&port.id) {
        findings.error(
            "L2R-S002",
            subject,
            "port id must be a lower-snake-case identifier usable in a Rust function name",
        );
    }
    if RESERVED_IDS.contains(&port.id.as_str()) {
        findings.error(
            "L2R-S010",
            subject,
            format!("the identifier `{}` is reserved", port.id),
        );
    }
    if !ids.insert(port.id.as_str()) {
        findings.error("L2R-P001", subject, "duplicate port id");
    }
    if model.component(&port.id).is_some() {
        findings.error(
            "L2R-P006",
            subject,
            "a port and a component cannot share an identifier: message edges name both",
        );
    }
    if !is_composite_id(model, &port.composite) {
        findings.error(
            "L2R-P002",
            subject,
            format!("`{}` is not a composite/v2 component", port.composite),
        );
    }
}

fn is_composite_id(model: &Model, id: &str) -> bool {
    model.component(id).is_some_and(is_composite)
}

fn check_in_port(model: &Model, port: &Port, subject: &str, findings: &mut Findings) {
    if port.capacity.is_some() {
        findings.error(
            "L2R-P008",
            subject,
            "`capacity` applies only to `out` ports of the root composite; an `in` port has no buffer of its own",
        );
    }
    check_in_target(model, port, subject, findings);
}

fn check_out_port(
    model: &Model,
    port: &Port,
    subject: &str,
    on_root: bool,
    findings: &mut Findings,
) {
    if port.to.is_some() || port.translation.is_some() {
        findings.error(
            "L2R-P009",
            subject,
            "an `out` port has no `to` or `translation`; message edges inside its composite feed it",
        );
    }
    let capacity_problem = match (on_root, port.capacity) {
        (true, None) => {
            Some("an `out` port of the root composite must state the `capacity` of its outbox".to_string())
        }
        (true, Some(n)) if n == 0 || n > MAX_MAILBOX_CAPACITY => Some(format!(
            "`capacity` must be between 1 and {MAX_MAILBOX_CAPACITY}"
        )),
        (false, Some(_)) => Some(
            "only the root composite's `out` ports have an outbox; this port forwards to the edges of its parent"
                .to_string(),
        ),
        _ => None,
    };
    if let Some(problem) = capacity_problem {
        findings.error("L2R-P008", subject, problem);
    }
    let read = model.messages.iter().any(|e| e.from == port.id);
    if !on_root && is_composite_id(model, &port.composite) && !read {
        findings.error(
            "L2R-M007",
            subject,
            "has no outgoing message edge in the parent composite, so every message reaching it would be discarded",
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
    let scope = source_scope(model, edge, subject, findings);
    let Some(target_scope) = target_scope(model, edge, subject, findings) else {
        return;
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

/// `L2R-M003` when `id` names a component (a composite), `L2R-M002` when it names nothing.
fn missing_endpoint_code(model: &Model, id: &str) -> &'static str {
    if model.component(id).is_some() {
        "L2R-M003"
    } else {
        "L2R-M002"
    }
}

/// The composite an edge lives in: where its source is visible. `None` (with a
/// finding) when the source cannot be an edge's source.
fn source_scope<'a>(
    model: &'a Model,
    edge: &MessageEdge,
    subject: &str,
    findings: &mut Findings,
) -> Option<&'a str> {
    let problem = match endpoint(model, &edge.from) {
        Some(Endpoint::Machine(machine)) => return machine.parent.as_deref(),
        None => (
            missing_endpoint_code(model, &edge.from),
            "is not a machine or an `out` port of a child composite",
        ),
        Some(Endpoint::Port(port)) if port.direction.as_deref() != Some("out") => (
            "L2R-M003",
            "is an `in` port; an `in` port forwards through its own `to`, not through edges",
        ),
        Some(Endpoint::Port(port)) => match parent_of(model, &port.composite) {
            Some(parent) => return Some(parent),
            None => (
                "L2R-M003",
                "is an `out` port of the root composite; its messages leave the system",
            ),
        },
    };
    findings.error(
        problem.0,
        subject,
        format!("from `{}` {}", edge.from, problem.1),
    );
    None
}

/// The composite in which an edge's target is visible. The outer `None` (with
/// a finding) means the target names nothing an edge can reach.
fn target_scope<'a>(
    model: &'a Model,
    edge: &MessageEdge,
    subject: &str,
    findings: &mut Findings,
) -> Option<Option<&'a str>> {
    match endpoint(model, &edge.to) {
        None => {
            findings.error(
                missing_endpoint_code(model, &edge.to),
                subject,
                format!("to `{}` is not a machine or a port", edge.to),
            );
            None
        }
        Some(Endpoint::Machine(machine)) => Some(machine.parent.as_deref()),
        Some(Endpoint::Port(port)) if port.direction.as_deref() == Some("out") => {
            Some(Some(port.composite.as_str()))
        }
        Some(Endpoint::Port(port)) => Some(parent_of(model, &port.composite)),
    }
}

/// Names the root's generated code derives from identifiers must be distinct:
/// the type prefixes (`a_1` and `a1` both give `A1`), the methods and fields of
/// `System` (`send_q` of port `q` against `<m>_state` of a machine `send_q`…),
/// and the functions of each `routes.rs`.
fn check_generated_names(model: &Model, findings: &mut Findings) {
    let root = root_id(model);
    let machines: Vec<(String, &str)> = model
        .components
        .iter()
        .filter(|c| is_machine(c))
        .map(|c| (subject(c), c.id.as_str()))
        .collect();
    let root_ports: Vec<(String, &str, bool)> = model
        .ports
        .iter()
        .filter(|p| root == Some(p.composite.as_str()))
        .map(|p| (port_subject(p), p.id.as_str(), p.is_out()))
        .collect();
    check_type_prefixes(&machines, &root_ports, findings);
    check_system_members(&machines, &root_ports, findings);
    check_route_functions(model, findings);
}

/// `a_1` and `a1` both give the type prefix `A1`.
fn check_type_prefixes(
    machines: &[(String, &str)],
    root_ports: &[(String, &str, bool)],
    findings: &mut Findings,
) {
    let mut types: BTreeMap<String, &String> = BTreeMap::new();
    let ids = machines
        .iter()
        .map(|(s, id)| (s, *id))
        .chain(root_ports.iter().map(|(s, id, _)| (s, *id)));
    for (subject, id) in ids {
        let type_name = names::upper_camel(id).unwrap_or_default();
        if let Some(other) = types.insert(type_name.clone(), subject) {
            findings.error(
                "L2R-S020",
                subject.as_str(),
                format!("generates the same Rust type prefix `{type_name}` as {other}"),
            );
        }
    }
}

/// Every method and field name of the generated `System`, with who it is for.
fn system_members<'a>(
    machines: &'a [(String, &str)],
    root_ports: &'a [(String, &str, bool)],
) -> Vec<(String, &'a str)> {
    let mut members: Vec<(String, &str)> = Vec::new();
    for (subject, id) in machines {
        for name in [
            format!("{id}_state"),
            format!("{id}_inbox"),
            format!("attempt_{id}"),
        ] {
            members.push((name, subject.as_str()));
        }
    }
    for (subject, id, is_out) in root_ports {
        let names = if *is_out {
            vec![format!("{id}_outbox"), format!("take_{id}")]
        } else {
            vec![format!("send_{id}")]
        };
        members.extend(names.into_iter().map(|name| (name, subject.as_str())));
    }
    for name in ["attempt", "is_settled", "new", "next", "run", "step_once"] {
        members.push((name.to_string(), "the generated `System` itself"));
    }
    members
}

fn check_system_members(
    machines: &[(String, &str)],
    root_ports: &[(String, &str, bool)],
    findings: &mut Findings,
) {
    let members = system_members(machines, root_ports);
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for (name, subject) in &members {
        let previous = seen.insert(name.as_str(), subject);
        if let Some(other) = previous.filter(|other| other != subject) {
            findings.error(
                "L2R-S020",
                *subject,
                format!("generates the same `System` member `{name}` as {other}"),
            );
        }
    }
}

/// In `routes.rs` an edge is the function `<id>` and an input port `port_<id>`.
fn check_route_functions(model: &Model, findings: &mut Findings) {
    for edge in &model.messages {
        let port = edge
            .id
            .strip_prefix("port_")
            .filter(|port| model.ports.iter().any(|p| p.id == *port));
        if let Some(port) = port {
            findings.error(
                "L2R-S020",
                format!("message `{}`", edge.id),
                format!("generates the same function name as port `{port}`"),
            );
        }
    }
}

/// Nothing approved may be dead: an edge or a nested input port that no
/// message can ever pass would be type-checked and documented, but generate
/// nothing, and so mislead a reader of the model.
fn check_reachable(model: &Model, flat: &flat::Flat, findings: &mut Findings) {
    let used: BTreeSet<&str> = flat
        .leaves
        .iter()
        .flat_map(|l| l.routes.iter().flat_map(|r| r.hops.iter()))
        .chain(flat.inputs.iter().flat_map(|i| i.hops.iter()))
        .map(|h| h.id.as_str())
        .collect();
    for edge in &model.messages {
        if !used.contains(edge.id.as_str()) {
            findings.error(
                "L2R-M010",
                format!("message `{}`", edge.id),
                format!(
                    "can never carry a message: nothing feeds `{}`; remove the edge or connect its source",
                    edge.from
                ),
            );
        }
    }
    for port in model.ports.iter().filter(|p| !p.is_out()) {
        if !used.contains(port.id.as_str()) {
            findings.error(
                "L2R-P010",
                port_subject(port),
                "can never carry a message: no input port or edge of the parent composite feeds it",
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
