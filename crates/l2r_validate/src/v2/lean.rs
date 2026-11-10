//! Lean reference rules of schema `v2`: every behavioral name and every
//! translation resolves in the inventory with the type the model implies, and
//! every Lean type the model names is supported data.

use std::collections::BTreeMap;

use l2r_model::{Component, DeclKind, INVENTORY_SCHEMA_V1, Inventory, MessageEdge, Model, Port};

use super::{Endpoint, edge_subject, endpoint, port_subject};
use crate::data::{self, DataType, TypeUse};
use crate::diagnostic::Findings;
use crate::lean_refs::{Refs, port_translation_type, step_type, translation_type};
use crate::structure::subject;

/// Model schema `v2` reads constructor fields, which only inventory `v1`
/// records. Returns `true` when the inventory is usable.
fn check_inventory_schema(inventory: &Inventory, findings: &mut Findings) -> bool {
    if inventory.schema == INVENTORY_SCHEMA_V1 {
        return true;
    }
    findings.error(
        "L2R-L001",
        "lean inventory",
        format!(
            "model schema v2 needs an inventory with constructor fields: expected `{INVENTORY_SCHEMA_V1}`, found `{}` (export with `--v1`)",
            inventory.schema
        ),
    );
    false
}

/// Resolves every Lean reference; returns the data types used.
pub(super) fn check(
    model: &Model,
    inventory: &Inventory,
    findings: &mut Findings,
) -> BTreeMap<String, DataType> {
    if !check_inventory_schema(inventory, findings) {
        return BTreeMap::new();
    }
    for component in &model.components {
        check_behavior_refs(component, inventory, findings);
    }
    for port in &model.ports {
        check_port_translation(model, port, inventory, findings);
    }
    for edge in &model.messages {
        check_edge_translation(model, edge, inventory, findings);
    }
    data::resolve(&type_uses(model), inventory, findings)
}

/// Every Lean type the model names: the state, input and output of each
/// machine, then the message of each port.
fn type_uses(model: &Model) -> Vec<TypeUse<'_>> {
    let of_machines = model.components.iter().flat_map(behavior_type_uses);
    let of_ports = model.ports.iter().map(|port| TypeUse {
        subject: port_subject(port),
        role: "message",
        lean_name: &port.message,
    });
    of_machines.chain(of_ports).collect()
}

fn behavior_type_uses(component: &Component) -> Vec<TypeUse<'_>> {
    let Some(behavior) = &component.behavior else {
        return Vec::new();
    };
    [
        ("owned_state", &behavior.owned_state),
        ("input", &behavior.input),
        ("output", &behavior.output),
    ]
    .into_iter()
    .map(|(role, name)| TypeUse {
        subject: subject(component),
        role,
        lean_name: name,
    })
    .collect()
}

/// A machine's `init`, `step` and invariants exist in Lean, with the types its
/// state, input and output give them.
fn check_behavior_refs(component: &Component, inventory: &Inventory, findings: &mut Findings) {
    let Some(behavior) = &component.behavior else {
        return;
    };
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

/// An input port's translation is total, from its message to what its target
/// takes.
fn check_port_translation(
    model: &Model,
    port: &Port,
    inventory: &Inventory,
    findings: &mut Findings,
) {
    let (Some(target), Some(translation)) = (port.to.as_deref(), port.translation.as_deref())
    else {
        return;
    };
    let Some(target_type) = endpoint(model, target).and_then(Endpoint::input) else {
        return;
    };
    let refs = Refs {
        subject: port_subject(port),
        inventory,
    };
    refs.expect_typed(
        translation,
        "translation",
        DeclKind::Definition,
        &port_translation_type(&port.message, target_type),
        findings,
    );
}

/// An edge's translation is partial, from what its source gives to what its
/// target takes.
fn check_edge_translation(
    model: &Model,
    edge: &MessageEdge,
    inventory: &Inventory,
    findings: &mut Findings,
) {
    let from = endpoint(model, &edge.from).and_then(Endpoint::output);
    let to = endpoint(model, &edge.to).and_then(Endpoint::input);
    let (Some(from), Some(to)) = (from, to) else {
        return;
    };
    let refs = Refs {
        subject: edge_subject(edge),
        inventory,
    };
    refs.expect_typed(
        &edge.translation,
        "translation",
        DeclKind::Definition,
        &translation_type(from, to),
        findings,
    );
}
