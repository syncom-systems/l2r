//! Message edge rules: who may send to whom, and what the message means.
//!
//! Hierarchy rule (the strictest option; the alternatives are a pending decision in ADR-0003): a message
//! edge may only connect two machines with the same parent. Nothing may reach
//! into another subtree, and a parent does not exchange messages with its own
//! children through edges.

use std::collections::BTreeSet;

use l2r_model::{DeclKind, Inventory, MessageEdge, Model, Profile};

use crate::diagnostic::Findings;
use crate::lean_refs::{Refs, translation_type};
use crate::names;

/// Applies every message edge rule.
pub fn check(model: &Model, inventory: Option<&Inventory>, findings: &mut Findings) {
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
        if check_endpoints(model, edge, &subject, findings)
            && let Some(inventory) = inventory
        {
            check_translation(model, inventory, edge, subject, findings);
        }
    }
}

/// Returns `true` when both endpoints exist and are permitted to communicate.
fn check_endpoints(
    model: &Model,
    edge: &MessageEdge,
    subject: &str,
    findings: &mut Findings,
) -> bool {
    let mut ok = true;
    for (role, id) in [("from", &edge.from), ("to", &edge.to)] {
        match model.component(id) {
            None => {
                findings.error("L2R-M002", subject, format!("{role} `{id}` does not exist"));
                ok = false;
            }
            Some(c) if c.parsed_profile() != Some(Profile::TransitionCoreV0) => {
                findings.error("L2R-M003", subject, format!("{role} `{id}` is not a state machine; only machines send and receive messages"));
                ok = false;
            }
            Some(_) => {}
        }
    }
    if edge.from == edge.to {
        findings.error(
            "L2R-M004",
            subject,
            "a component cannot message itself; express this inside its transition function",
        );
        ok = false;
    }
    if ok {
        let from_parent = model
            .component(&edge.from)
            .and_then(|c| c.parent.as_deref());
        let to_parent = model.component(&edge.to).and_then(|c| c.parent.as_deref());
        if from_parent != to_parent {
            findings.error(
                "L2R-M005",
                subject,
                format!(
                    "`{}` and `{}` have different parents; messages may only connect siblings",
                    edge.from, edge.to
                ),
            );
            ok = false;
        }
    }
    ok
}

fn check_translation(
    model: &Model,
    inventory: &Inventory,
    edge: &MessageEdge,
    subject: String,
    findings: &mut Findings,
) {
    let behavior_of = |id: &str| model.component(id).and_then(|c| c.behavior.as_ref());
    let (Some(from), Some(to)) = (behavior_of(&edge.from), behavior_of(&edge.to)) else {
        return;
    };
    let expected = translation_type(&from.output, &to.input);
    let refs = Refs { subject, inventory };
    refs.expect_typed(
        &edge.translation,
        "translation",
        DeclKind::Definition,
        &expected,
        findings,
    );
}
