//! Rules of model schema `l2r.architecture/v2`: hierarchical composition.
//!
//! A `v2` model has machines (`transition-core/v1`) and composites
//! (`composite/v2`) nested to any depth. Messages still connect siblings only:
//! an edge lives in one composite and joins two things visible there, namely
//! its child machines, the ports of its child composites, and its own ports.
//! Crossing a composite's boundary therefore always goes through a typed port
//! that the architect declared.
//!
//! Each family of rules has its own module. This one holds what they share,
//! the vocabulary of a `v2` model, and the rules of the flat network.

mod components;
mod edges;
mod generated_names;
mod lean;
mod ports;

use std::collections::BTreeSet;

use l2r_model::{Component, Inventory, MessageEdge, Model, Port, profiles};

use crate::diagnostic::{Diagnostic, Findings};
use crate::flat::{self, Flat, FlatError};
use crate::lean_refs;
use crate::resolved::Validated;
use crate::structure;

/// Identifiers the generated `System` uses as prefixes of its own methods.
const RESERVED_IDS: &[&str] = &["attempt", "send", "take"];

/// Validates a canonical `v2` model against its inventory.
pub fn validate(model: Model, inventory: Inventory) -> Result<Validated, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    structure::check_common(&model, &mut findings);
    components::check(&model, &mut findings);
    ports::check(&model, &mut findings);
    edges::check(&model, &mut findings);
    generated_names::check(&model, &mut findings);
    lean_refs::check_ownership(&model, &mut findings);
    let types = lean::check(&model, &inventory, &mut findings);

    // The flat network is defined only for a structurally valid model.
    let flat = if findings.is_empty() {
        flat_network(&model, &mut findings)
    } else {
        None
    };

    match flat {
        Some(flat) if findings.is_empty() => Ok(Validated::new_v2(model, inventory, types, flat)),
        _ => Err(findings.into_sorted()),
    }
}

/// The flat network of a structurally valid model, with a finding for every
/// reason it has none and for everything approved that it never uses.
fn flat_network(model: &Model, findings: &mut Findings) -> Option<Flat> {
    match flat::flatten(model) {
        Ok(flat) => {
            check_reachable(model, &flat, findings);
            Some(flat)
        }
        Err(errors) => {
            for error in errors {
                report_flat_error(error, findings);
            }
            None
        }
    }
}

fn report_flat_error(error: FlatError, findings: &mut Findings) {
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

fn is_machine(component: &Component) -> bool {
    component.profile == profiles::TRANSITION_CORE_V1
}

fn is_composite(component: &Component) -> bool {
    component.profile == profiles::COMPOSITE_V2
}

fn port_subject(port: &Port) -> String {
    format!("port `{}`", port.id)
}

fn edge_subject(edge: &MessageEdge) -> String {
    format!("message `{}`", edge.id)
}

fn root(model: &Model) -> Option<&Component> {
    model.components.iter().find(|c| c.parent.is_none())
}

fn root_id(model: &Model) -> Option<&str> {
    root(model).map(|c| c.id.as_str())
}

fn has_outgoing_edge(model: &Model, id: &str) -> bool {
    model.messages.iter().any(|e| e.from == id)
}

/// What an edge endpoint names.
#[derive(Clone, Copy)]
enum Endpoint<'a> {
    Machine(&'a Component),
    Port(&'a Port),
}

impl<'a> Endpoint<'a> {
    /// Lean type of the messages the endpoint takes.
    fn input(self) -> Option<&'a str> {
        match self {
            Endpoint::Machine(machine) => machine.behavior.as_ref().map(|b| b.input.as_str()),
            Endpoint::Port(port) => Some(port.message.as_str()),
        }
    }

    /// Lean type of the messages the endpoint gives.
    fn output(self) -> Option<&'a str> {
        match self {
            Endpoint::Machine(machine) => machine.behavior.as_ref().map(|b| b.output.as_str()),
            Endpoint::Port(port) => Some(port.message.as_str()),
        }
    }
}

fn endpoint<'a>(model: &'a Model, id: &str) -> Option<Endpoint<'a>> {
    if let Some(component) = model.component(id) {
        return is_machine(component).then_some(Endpoint::Machine(component));
    }
    model.ports.iter().find(|p| p.id == id).map(Endpoint::Port)
}

/// The edges and ports some message of the flat network passes.
fn carrying_hops(flat: &Flat) -> BTreeSet<&str> {
    let routes = flat.leaves.iter().flat_map(|l| &l.routes);
    let of_routes = routes.flat_map(|r| &r.hops);
    let of_inputs = flat.inputs.iter().flat_map(|i| &i.hops);
    of_routes.chain(of_inputs).map(|h| h.id.as_str()).collect()
}

/// Nothing approved may be dead: an edge or a nested input port that no
/// message can ever pass would be type-checked and documented, but generate
/// nothing, and so mislead a reader of the model.
fn check_reachable(model: &Model, flat: &Flat, findings: &mut Findings) {
    let used = carrying_hops(flat);
    let dead_edges = model
        .messages
        .iter()
        .filter(|e| !used.contains(e.id.as_str()));
    for edge in dead_edges {
        findings.error(
            "L2R-M010",
            edge_subject(edge),
            format!(
                "can never carry a message: nothing feeds `{}`; remove the edge or connect its source",
                edge.from
            ),
        );
    }
    let dead_inputs = model
        .ports
        .iter()
        .filter(|p| !p.is_out() && !used.contains(p.id.as_str()));
    for port in dead_inputs {
        findings.error(
            "L2R-P010",
            port_subject(port),
            "can never carry a message: no input port or edge of the parent composite feeds it",
        );
    }
}
