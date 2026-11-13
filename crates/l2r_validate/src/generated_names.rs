//! Name rules of schemas `v0` and `v1`: no two names a composite's generated
//! code declares may meet in one Rust namespace, none may hide a standard name
//! that code uses, and distinct Lean types must stay distinct Rust types.
//!
//! These rules cover the names only. Other reasons generated code could fail to
//! compile are not checked here.
//!
//! The names themselves are decided by [`l2r_model::rust_names`]; this module
//! asks for them and reports why a set cannot exist. A model that passes keeps
//! its names in [`crate::Validated`], so the planner emits exactly the names
//! checked here.

use std::collections::BTreeMap;

use l2r_model::rust_names::{Owner, Problem, composite_v0, composite_v1};
use l2r_model::{Component, Model, profiles};

use crate::diagnostic::Findings;
use crate::names;
use crate::structure::subject;

/// The generated names of one `v0`/`v1` composite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompositeNames {
    V0(composite_v0::SystemNames),
    V1(composite_v1::SystemNames),
}

/// Computes the names of every `composite/v0` and `composite/v1` in `model`
/// and reports each problem: `L2R-P005` when a Lean type a port carries is
/// involved, `L2R-S021` otherwise.
pub(crate) fn check(model: &Model, findings: &mut Findings) -> BTreeMap<String, CompositeNames> {
    let mut all = BTreeMap::new();
    for composite in &model.components {
        if !well_formed(model, composite) {
            continue;
        }
        let names = match composite.profile.as_str() {
            profiles::COMPOSITE_V0 => v0_names(model, composite).map(CompositeNames::V0),
            profiles::COMPOSITE_V1 => v1_names(model, composite).map(CompositeNames::V1),
            _ => continue,
        };
        match names {
            Ok(names) => {
                all.insert(composite.id.clone(), names);
            }
            Err(problems) => report(model, composite, &problems, findings),
        }
    }
    all
}

/// True when the identifiers and Lean names the composite's names derive from
/// are well formed and its children's modules distinct. Otherwise other rules
/// (S002, S009, S010, L002) already reject the model, and a name conflict
/// reported on top of them would describe the same mistake again.
fn well_formed(model: &Model, composite: &Component) -> bool {
    let children = model.children(&composite.id);
    distinct_modules(&children)
        && children
            .iter()
            .all(|c| names::is_identifier(&c.id) && is_module_name(&c.rust_module))
        && edge_ids(model, composite).into_iter().all(is_item_name)
        && ports_of(model, composite)
            .all(|p| is_item_name(&p.id) && names::is_lean_name(&p.message))
}

fn distinct_modules(children: &[&Component]) -> bool {
    let mut modules: Vec<&str> = children.iter().map(|c| c.rust_module.as_str()).collect();
    modules.sort_unstable();
    modules.windows(2).all(|pair| pair[0] != pair[1])
}

/// The syntax S002 and S010 require of a `rust_module`. A component id needs
/// only S002's (`self` passes it, and is this module's to reject).
fn is_module_name(module: &str) -> bool {
    is_item_name(module) && !names::RESERVED_MODULES.contains(&module)
}

/// The syntax S002 requires of an edge or port id.
fn is_item_name(id: &str) -> bool {
    names::is_identifier(id) && !names::is_rust_keyword(id)
}

fn ports_of<'a>(
    model: &'a Model,
    composite: &'a Component,
) -> impl Iterator<Item = &'a l2r_model::Port> {
    model
        .ports
        .iter()
        .filter(move |p| p.composite == composite.id)
}

fn v0_names(
    model: &Model,
    composite: &Component,
) -> Result<composite_v0::SystemNames, Vec<Problem>> {
    let children: Vec<composite_v0::Child<'_>> = model
        .children(&composite.id)
        .into_iter()
        .map(|c| composite_v0::Child {
            id: &c.id,
            module: &c.rust_module,
            is_machine: c.profile == profiles::TRANSITION_CORE_V0,
        })
        .collect();
    composite_v0::SystemNames::new(&children, &edge_ids(model, composite))
}

fn v1_names(
    model: &Model,
    composite: &Component,
) -> Result<composite_v1::SystemNames, Vec<Problem>> {
    let children: Vec<composite_v1::Child<'_>> = model
        .children(&composite.id)
        .into_iter()
        .map(|c| composite_v1::Child {
            id: &c.id,
            module: &c.rust_module,
        })
        .collect();
    let ports: Vec<composite_v1::Port<'_>> = ports_of(model, composite)
        .map(|p| composite_v1::Port {
            id: &p.id,
            message: &p.message,
        })
        .collect();
    composite_v1::SystemNames::new(&children, &ports, &edge_ids(model, composite))
}

fn edge_ids<'a>(model: &'a Model, composite: &Component) -> Vec<&'a str> {
    model
        .edges_within(&composite.id)
        .into_iter()
        .map(|e| e.id.as_str())
        .collect()
}

fn report(model: &Model, composite: &Component, problems: &[Problem], findings: &mut Findings) {
    for problem in problems {
        let code = if problem.involves_lean_type() {
            "L2R-P005"
        } else {
            "L2R-S021"
        };
        let subject = match problem.subject() {
            Owner::Component(id) => format!("component `{id}`"),
            Owner::Port(id) => format!("port `{id}`"),
            Owner::Edge(id) => format!("message `{id}`"),
            Owner::LeanType(name) => carrier(model, composite, name),
            Owner::Profile | Owner::Std => subject(composite),
        };
        findings.error(code, &subject, problem.to_string());
    }
}

/// The first port of `composite` that carries Lean type `name`.
fn carrier(model: &Model, composite: &Component, name: &str) -> String {
    model
        .ports
        .iter()
        .find(|p| p.composite == composite.id && p.message == name)
        .map(|p| format!("port `{}`", p.id))
        .unwrap_or_else(|| subject(composite))
}
