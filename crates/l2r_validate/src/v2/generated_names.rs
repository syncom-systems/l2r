//! Name rules of schema `v2`: identifiers the architect chose apart must not
//! meet again as one name in the generated code.

use std::collections::BTreeMap;

use l2r_model::Model;
use l2r_model::rust_names;

use super::{edge_subject, is_machine, port_subject, root_id};
use crate::diagnostic::Findings;
use crate::structure::subject;

/// Names the root's generated code derives from identifiers must be distinct:
/// the type prefixes (`a_1` and `a1` both give `A1`), the methods and fields of
/// `System` (`send_q` of port `q` against `<m>_state` of a machine `send_q`…),
/// and the functions of each `routes.rs`.
pub(super) fn check(model: &Model, findings: &mut Findings) {
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
        let type_name = rust_names::rust_type_name(id).unwrap_or_default();
        if let Some(other) = types.insert(type_name.clone(), subject) {
            findings.error(
                "L2R-S020",
                subject.as_str(),
                format!("generates the same Rust type prefix `{type_name}` as {other}"),
            );
        }
    }
}

/// What the generated `System` names for itself.
const SYSTEM_OWN_MEMBERS: &[&str] = &["attempt", "is_settled", "new", "next", "run", "step_once"];

/// The `System` members generated for machine `id`.
fn machine_members(id: &str) -> Vec<String> {
    vec![
        format!("{id}_state"),
        format!("{id}_inbox"),
        format!("attempt_{id}"),
    ]
}

/// The `System` members generated for root port `id`.
fn root_port_members(id: &str, is_out: bool) -> Vec<String> {
    if is_out {
        vec![format!("{id}_outbox"), format!("take_{id}")]
    } else {
        vec![format!("send_{id}")]
    }
}

/// Every method and field name of the generated `System`, with who it is for.
fn system_members<'a>(
    machines: &'a [(String, &str)],
    root_ports: &'a [(String, &str, bool)],
) -> Vec<(String, &'a str)> {
    let of_machines = machines.iter().flat_map(|(subject, id)| {
        let members = machine_members(id).into_iter();
        members.map(move |name| (name, subject.as_str()))
    });
    let of_root_ports = root_ports.iter().flat_map(|(subject, id, is_out)| {
        let members = root_port_members(id, *is_out).into_iter();
        members.map(move |name| (name, subject.as_str()))
    });
    let own = SYSTEM_OWN_MEMBERS
        .iter()
        .map(|name| (name.to_string(), "the generated `System` itself"));
    of_machines.chain(of_root_ports).chain(own).collect()
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
                edge_subject(edge),
                format!("generates the same function name as port `{port}`"),
            );
        }
    }
}
