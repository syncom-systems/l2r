//! Name rules of schema `v2`: identifiers the architect chose apart must not
//! meet again as one name in the generated code.

use std::collections::BTreeMap;

use l2r_model::Model;
use l2r_model::rust_names::{self, composite_v2};

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
        // `None` for an identifier S002 already rejects; the empty prefix keeps
        // this rule's findings for such models what they always were.
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

/// Every method and field name of the generated `System`, with who it is for.
fn system_members<'a>(
    machines: &'a [(String, &str)],
    root_ports: &'a [(String, &str, bool)],
) -> Vec<(String, &'a str)> {
    let of_machines = machines.iter().flat_map(|(subject, id)| {
        let members = composite_v2::Machine(id).members().into_iter();
        members.map(move |name| (name, subject.as_str()))
    });
    let of_root_ports = root_ports.iter().flat_map(|(subject, id, is_out)| {
        let members = if *is_out {
            composite_v2::OutPort(id).members()
        } else {
            composite_v2::InPort(id).members()
        }
        .into_iter();
        members.map(move |name| (name, subject.as_str()))
    });
    let own = composite_v2::SYSTEM_MEMBERS
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
        let function = composite_v2::Edge(&edge.id).function();
        let port = model
            .ports
            .iter()
            .find(|p| composite_v2::InPort(&p.id).function() == function);
        if let Some(port) = port.map(|p| &p.id) {
            findings.error(
                "L2R-S020",
                edge_subject(edge),
                format!("generates the same function name as port `{port}`"),
            );
        }
    }
}
