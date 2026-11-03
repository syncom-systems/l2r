//! Port rules for `composite/v1`: the typed interface between a composite and
//! its environment.
//!
//! A port feeds exactly one child machine, carries a Lean message type and a
//! total Lean translation to that child's input. The structural contract of the
//! interface is therefore the set of ports and their types: nothing else can
//! reach a child from outside. When events may occur is a separate question,
//! answered by an environment assumption in the system proofs.

use std::collections::{BTreeMap, BTreeSet};

use l2r_model::{DeclKind, Inventory, Model, Port, profiles};

use crate::diagnostic::Findings;
use crate::lean_refs::{Refs, port_translation_type};
use crate::names;
use crate::resolved::ResolvedEnum;

/// Applies the structural port rules.
pub fn check(model: &Model, findings: &mut Findings) {
    let mut ids = BTreeSet::new();
    for port in &model.ports {
        let subject = format!("port `{}`", port.id);
        if !names::is_identifier(&port.id) || names::is_rust_keyword(&port.id) {
            findings.error(
                "L2R-S002",
                &subject,
                "port id must be a lower-snake-case identifier usable in a Rust function name",
            );
        }
        if !ids.insert(port.id.as_str()) {
            findings.error("L2R-P001", &subject, "duplicate port id");
        }
        match model.component(&port.composite) {
            Some(c) if c.profile == profiles::COMPOSITE_V1 => {}
            Some(_) => findings.error(
                "L2R-P002",
                &subject,
                format!(
                    "`{}` is not a composite/v1; only it has ports",
                    port.composite
                ),
            ),
            None => findings.error(
                "L2R-P002",
                &subject,
                format!("composite `{}` does not exist", port.composite),
            ),
        }
        match model.component(&port.to) {
            Some(child)
                if child.profile == profiles::TRANSITION_CORE_V0
                    && child.parent.as_deref() == Some(port.composite.as_str()) => {}
            _ => findings.error(
                "L2R-P003",
                &subject,
                format!(
                    "`{}` is not a machine inside composite `{}`",
                    port.to, port.composite
                ),
            ),
        }
    }
    for composite in model
        .components
        .iter()
        .filter(|c| c.profile == profiles::COMPOSITE_V1)
    {
        let own: Vec<&Port> = model
            .ports
            .iter()
            .filter(|p| p.composite == composite.id)
            .collect();
        if own.is_empty() {
            findings.error(
                "L2R-P004",
                format!("component `{}`", composite.id),
                "a composite/v1 without ports can receive nothing from outside",
            );
        }
        // No silent discard: v1 has no output ports yet, so a machine whose
        // outputs nobody consumes is rejected rather than dropping them.
        for child in model.children(&composite.id) {
            if !model.messages.iter().any(|e| e.from == child.id) {
                findings.error(
                    "L2R-M007",
                    format!("component `{}`", child.id),
                    "has no outgoing message edge, so every output it emits would be discarded; \
                     composite/v1 never discards silently, and has no output ports yet",
                );
            }
        }
    }
}

/// Resolves the Lean references of every port.
pub fn resolve(
    model: &Model,
    inventory: &Inventory,
    findings: &mut Findings,
) -> BTreeMap<String, ResolvedEnum> {
    let mut resolved = BTreeMap::new();
    for port in &model.ports {
        let refs = Refs {
            subject: format!("port `{}`", port.id),
            inventory,
        };
        let message = refs.resolve_enum(&port.message, "message", findings);
        if let Some(input) = model
            .component(&port.to)
            .and_then(|c| c.behavior.as_ref())
            .map(|b| b.input.as_str())
        {
            refs.expect_typed(
                &port.translation,
                "translation",
                DeclKind::Definition,
                &port_translation_type(&port.message, input),
                findings,
            );
        }
        if let Some(message) = message {
            resolved.insert(port.id.clone(), message);
        }
    }
    resolved
}
