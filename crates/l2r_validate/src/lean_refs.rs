//! Lean reference rules: every behavioral name in the model must resolve, with the
//! expected kind and type, against the Lean declaration inventory.

use std::collections::{BTreeMap, BTreeSet};

use l2r_model::rust_names;
use l2r_model::{Behavior, Component, DeclKind, Declaration, INVENTORY_SCHEMA, Inventory, Model};

use crate::diagnostic::Findings;
use crate::names;
use crate::resolved::{ResolvedEnum, ResolvedMachine, Variant};
use crate::structure::subject;

/// Rejects inventories this generator cannot interpret. Returns `true` when usable.
pub fn check_inventory(inventory: &Inventory, findings: &mut Findings) -> bool {
    if inventory.schema != INVENTORY_SCHEMA {
        findings.error(
            "L2R-L001",
            "lean inventory",
            format!(
                "unsupported inventory schema `{}`; expected `{INVENTORY_SCHEMA}`",
                inventory.schema
            ),
        );
        return false;
    }
    true
}

/// The exact type a transition function must have, as printed by the exporter.
pub fn step_type(state: &str, input: &str, output: &str) -> String {
    format!("{state} → {input} → Prod {state} (Option {output})")
}

/// The exact type a message translation must have, as printed by the exporter.
pub fn translation_type(from_output: &str, to_input: &str) -> String {
    format!("{from_output} → Option {to_input}")
}

/// The exact type a port translation must have: total, unlike an edge's.
pub fn port_translation_type(message: &str, to_input: &str) -> String {
    format!("{message} → {to_input}")
}

/// Resolves the Lean references of every machine component.
pub fn resolve_machines(
    model: &Model,
    inventory: &Inventory,
    findings: &mut Findings,
) -> BTreeMap<String, ResolvedMachine> {
    model
        .components
        .iter()
        .filter_map(|component| {
            let behavior = component.behavior.as_ref()?;
            let resolved = resolve_machine(component, behavior, inventory, findings)?;
            Some((component.id.clone(), resolved))
        })
        .collect()
}

fn resolve_machine(
    component: &Component,
    behavior: &Behavior,
    inventory: &Inventory,
    findings: &mut Findings,
) -> Option<ResolvedMachine> {
    let refs = Refs {
        subject: subject(component),
        inventory,
    };
    let state = refs.resolve_enum(&behavior.owned_state, "owned_state", findings);
    let input = refs.resolve_enum(&behavior.input, "input", findings);
    let output = refs.resolve_enum(&behavior.output, "output", findings);
    refs.expect_typed(
        &behavior.init,
        "init",
        DeclKind::Definition,
        &behavior.owned_state,
        findings,
    );
    let step = step_type(&behavior.owned_state, &behavior.input, &behavior.output);
    refs.expect_typed(
        &behavior.step,
        "step",
        DeclKind::Definition,
        &step,
        findings,
    );
    for invariant in &behavior.invariants {
        refs.expect_decl(invariant, "invariant", DeclKind::Theorem, findings);
    }
    Some(ResolvedMachine {
        state: state?,
        input: input?,
        output: output?,
    })
}

/// Resolution context for one model element.
pub(crate) struct Refs<'a> {
    pub(crate) subject: String,
    pub(crate) inventory: &'a Inventory,
}

impl Refs<'_> {
    pub(crate) fn expect_decl(
        &self,
        name: &str,
        role: &str,
        kind: DeclKind,
        findings: &mut Findings,
    ) -> Option<&Declaration> {
        if !names::is_lean_name(name) {
            findings.error(
                "L2R-L002",
                &self.subject,
                format!("{role} `{name}` is not a qualified Lean name"),
            );
            return None;
        }
        let Some(decl) = self.inventory.find(name) else {
            findings.error(
                "L2R-L003",
                &self.subject,
                format!("{role} `{name}` is not in the Lean inventory"),
            );
            return None;
        };
        if decl.kind != kind {
            findings.error(
                "L2R-L004",
                &self.subject,
                format!(
                    "{role} `{name}` must be a Lean {kind:?}, but the inventory records a {:?}",
                    decl.kind
                ),
            );
            return None;
        }
        Some(decl)
    }

    pub(crate) fn expect_typed(
        &self,
        name: &str,
        role: &str,
        kind: DeclKind,
        expected: &str,
        findings: &mut Findings,
    ) {
        if let Some(decl) = self.expect_decl(name, role, kind, findings)
            && decl.type_ != expected
        {
            findings.error(
                "L2R-L006",
                &self.subject,
                format!(
                    "{role} `{name}` has type `{}`; expected `{expected}`",
                    decl.type_
                ),
            );
        }
    }

    pub(crate) fn resolve_enum(
        &self,
        name: &str,
        role: &str,
        findings: &mut Findings,
    ) -> Option<ResolvedEnum> {
        let decl = self.expect_decl(name, role, DeclKind::Inductive, findings)?;
        if decl.type_ != "Type"
            || decl.constructors.is_empty()
            || decl.constructors.iter().any(|c| c.arity != 0)
        {
            findings.error(
                "L2R-L005",
                &self.subject,
                format!("{role} `{name}` must be an inductive `Type` whose constructors have no fields (profile transition-core/v0)"),
            );
            return None;
        }
        let mut taken = BTreeSet::new();
        let mut variants = Vec::new();
        for ctor in &decl.constructors {
            let segment = rust_names::lean_last_segment(&ctor.name);
            let Some(rust_name) =
                rust_names::rust_type_name(segment).filter(|n| taken.insert(n.clone()))
            else {
                findings.error(
                    "L2R-L007",
                    &self.subject,
                    format!(
                        "constructor `{}` of {role} cannot be given a unique Rust variant name",
                        ctor.name
                    ),
                );
                return None;
            };
            variants.push(Variant {
                lean_name: ctor.name.clone(),
                rust_name,
            });
        }
        Some(ResolvedEnum {
            lean_name: name.to_string(),
            variants,
        })
    }
}

/// Each Lean state type has exactly one owning component.
pub fn check_ownership(model: &Model, findings: &mut Findings) {
    let mut owners: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for component in &model.components {
        if let Some(behavior) = &component.behavior {
            owners
                .entry(behavior.owned_state.as_str())
                .or_default()
                .push(component.id.as_str());
        }
    }
    for (state, owner_ids) in owners {
        if owner_ids.len() > 1 {
            findings.error(
                "L2R-O001",
                format!("state `{state}`"),
                format!(
                    "owned by more than one component: [{}]; state must have exactly one owner",
                    owner_ids.join(", ")
                ),
            );
        }
    }
}
