//! Validation of architectural models against their Lean inventory.
//!
//! Validation never repairs or guesses: an inconsistent model is rejected with
//! every finding listed, and only a fully valid model yields a [`Validated`].

pub mod data;
mod diagnostic;
pub mod flat;
mod generated_names;
mod lean_refs;
mod messages;
pub mod names;
mod ports;
mod resolved;
mod structure;
mod v2;

pub use diagnostic::Diagnostic;
pub use lean_refs::{port_translation_type, step_type, translation_type};
pub use resolved::{ResolvedEnum, ResolvedMachine, Validated, Variant};

use resolved::Resolution;

use l2r_model::{Inventory, MODEL_SCHEMA_V2, Model};

/// Validates `model` against `inventory`.
///
/// Both inputs are canonicalized first, so the result does not depend on the
/// order in which the architect wrote components, edges or invariants.
pub fn validate(model: &Model, inventory: &Inventory) -> Result<Validated, Vec<Diagnostic>> {
    let model = model.canonical();
    let inventory = inventory.canonical();
    if model.schema == MODEL_SCHEMA_V2 {
        return v2::validate(model, inventory);
    }
    let mut findings = diagnostic::Findings::default();

    structure::check(&model, &mut findings);
    lean_refs::check_ownership(&model, &mut findings);
    let inventory_ok = lean_refs::check_inventory(&inventory, &mut findings);
    let usable_inventory = inventory_ok.then_some(&inventory);
    messages::check(&model, usable_inventory, &mut findings);
    ports::check(&model, &mut findings);
    let composite_names = generated_names::check(&model, &mut findings);
    let machines = match usable_inventory {
        Some(inventory) => lean_refs::resolve_machines(&model, inventory, &mut findings),
        None => Default::default(),
    };
    let port_messages = match usable_inventory {
        Some(inventory) => ports::resolve(&model, inventory, &mut findings),
        None => Default::default(),
    };

    if findings.is_empty() {
        let resolution = Resolution {
            machines,
            port_messages,
            composite_names,
        };
        Ok(Validated::new(model, inventory, resolution))
    } else {
        Err(findings.into_sorted())
    }
}
