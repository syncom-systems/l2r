//! Edge rules of schema `v2`: an edge lives in one composite and joins two
//! things visible there, and no machine's output is left without a receiver.

use std::collections::BTreeSet;

use l2r_model::{MessageEdge, Model};

use super::{Endpoint, edge_subject, endpoint, has_outgoing_edge, is_machine};
use crate::diagnostic::Findings;
use crate::names;
use crate::structure::subject;

fn parent_of<'a>(model: &'a Model, id: &str) -> Option<&'a str> {
    model.component(id).and_then(|c| c.parent.as_deref())
}

pub(super) fn check(model: &Model, findings: &mut Findings) {
    let mut ids = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    for edge in &model.messages {
        let subject = edge_subject(edge);
        if !names::is_identifier(&edge.id) || names::is_rust_keyword(&edge.id) {
            findings.error(
                "L2R-S002",
                &subject,
                "message id must be a lower-snake-case identifier usable as a Rust function name",
            );
        }
        check_edge_is_new(edge, &mut ids, &mut pairs, findings);
        check_edge_endpoints(model, edge, &subject, findings);
    }
    check_machines_are_read(model, findings);
}

/// An edge repeats neither the identifier (`ids`) nor the source and target
/// (`pairs`) of an edge before it.
fn check_edge_is_new<'a>(
    edge: &'a MessageEdge,
    ids: &mut BTreeSet<&'a str>,
    pairs: &mut BTreeSet<(&'a str, &'a str)>,
    findings: &mut Findings,
) {
    if !ids.insert(edge.id.as_str()) {
        findings.error("L2R-M001", edge_subject(edge), "duplicate message id");
    }
    if !pairs.insert((edge.from.as_str(), edge.to.as_str())) {
        findings.error(
            "L2R-M006",
            edge_subject(edge),
            format!(
                "a second edge from `{}` to `{}` would make delivery ambiguous",
                edge.from, edge.to
            ),
        );
    }
}

/// Every machine's outputs go somewhere.
fn check_machines_are_read(model: &Model, findings: &mut Findings) {
    let unread = model
        .components
        .iter()
        .filter(|c| is_machine(c) && !has_outgoing_edge(model, &c.id));
    for machine in unread {
        findings.error(
            "L2R-M007",
            subject(machine),
            "has no outgoing message edge, so every output it emits would be discarded; route its outputs to a sibling or to an `out` port",
        );
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

/// Why something cannot be the source of an edge: the rule it breaks, and the
/// reason, worded as the rest of the sentence "from `<id>` …".
struct NotASource {
    code: &'static str,
    reason: &'static str,
}

/// The composite in which `id` is visible as the source of an edge.
fn visible_source<'a>(model: &'a Model, id: &str) -> Result<Option<&'a str>, NotASource> {
    match endpoint(model, id) {
        Some(Endpoint::Machine(machine)) => Ok(machine.parent.as_deref()),
        None => Err(NotASource {
            code: missing_endpoint_code(model, id),
            reason: "is not a machine or an `out` port of a child composite",
        }),
        Some(Endpoint::Port(port)) if !port.is_out() => Err(NotASource {
            code: "L2R-M003",
            reason: "is an `in` port; an `in` port forwards through its own `to`, not through edges",
        }),
        Some(Endpoint::Port(port)) => {
            parent_of(model, &port.composite)
                .map(Some)
                .ok_or(NotASource {
                    code: "L2R-M003",
                    reason: "is an `out` port of the root composite; its messages leave the system",
                })
        }
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
    match visible_source(model, &edge.from) {
        Ok(scope) => scope,
        Err(NotASource { code, reason }) => {
            findings.error(code, subject, format!("from `{}` {reason}", edge.from));
            None
        }
    }
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
        Some(Endpoint::Port(port)) if port.is_out() => Some(Some(port.composite.as_str())),
        Some(Endpoint::Port(port)) => Some(parent_of(model, &port.composite)),
    }
}
