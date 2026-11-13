//! Component rules of schema `v2`: every component is a machine or a
//! composite and has the shape of its kind, and the root is a composite.

use l2r_model::{Component, Model, Schema, profiles};

use super::MAX_CAPACITY;
use super::{RESERVED_IDS, is_machine, root};
use crate::diagnostic::Findings;
use crate::structure::subject;

pub(super) fn check(model: &Model, findings: &mut Findings) {
    for component in &model.components {
        check_component(model, component, findings);
    }
    check_root_is_composite(model, findings);
}

/// A component is a machine or a composite, and has the shape of its kind.
fn check_component(model: &Model, component: &Component, findings: &mut Findings) {
    let subject = subject(component);
    if !component
        .parsed_profile()
        .is_some_and(|p| Schema::V2.admits(p))
    {
        findings.error(
            "L2R-S019",
            &subject,
            format!(
                "profile `{}` cannot be used in a v2 model; use `{}` for machines and `{}` for composites",
                component.profile,
                profiles::TRANSITION_CORE_V1,
                profiles::COMPOSITE_V2
            ),
        );
        return;
    }
    if RESERVED_IDS.contains(&component.id.as_str()) {
        findings.error(
            "L2R-S010",
            &subject,
            format!("the identifier `{}` is reserved", component.id),
        );
    }
    let has_children = model
        .components
        .iter()
        .any(|c| c.parent.as_deref() == Some(&component.id));
    if is_machine(component) {
        check_machine_shape(component, has_children, findings);
    } else {
        check_composite_shape(component, has_children, findings);
    }
}

/// A machine has behavior and a bounded mailbox, and contains nothing.
fn check_machine_shape(component: &Component, has_children: bool, findings: &mut Findings) {
    let subject = subject(component);
    if component.behavior.is_none() {
        findings.error(
            "L2R-S008",
            &subject,
            "a machine requires a [component.behavior] table",
        );
    }
    if has_children {
        findings.error(
            "L2R-S008",
            &subject,
            "a machine cannot contain other components",
        );
    }
    if let Some(problem) = mailbox_capacity_problem(component.mailbox_capacity) {
        findings.error("L2R-S017", &subject, problem);
    }
}

/// Why a machine's `mailbox_capacity` is not a supported bound, if it is not.
fn mailbox_capacity_problem(capacity: Option<u32>) -> Option<String> {
    match capacity {
        None => Some("a machine must state `mailbox_capacity`".to_string()),
        Some(0) => Some("`mailbox_capacity` must be at least 1".to_string()),
        Some(n) if n > MAX_CAPACITY => Some(format!(
            "`mailbox_capacity` {n} exceeds the supported maximum {MAX_CAPACITY}"
        )),
        Some(_) => None,
    }
}

/// A composite only contains: it has children, and neither behavior nor a
/// mailbox of its own.
fn check_composite_shape(component: &Component, has_children: bool, findings: &mut Findings) {
    let subject = subject(component);
    if component.behavior.is_some() {
        findings.error(
            "L2R-S008",
            &subject,
            "a composite owns no behavioral state; remove [component.behavior]",
        );
    }
    if !has_children {
        findings.error(
            "L2R-S008",
            &subject,
            "a composite must contain at least one component",
        );
    }
    if component.mailbox_capacity.is_some() {
        findings.error(
            "L2R-S018",
            &subject,
            "`mailbox_capacity` applies only to machines",
        );
    }
    if component.parent.is_none() && component.rust_module == "types" {
        findings.error(
            "L2R-S010",
            &subject,
            "the root module cannot be `types`: the generated crate has a module of that name",
        );
    }
}

fn check_root_is_composite(model: &Model, findings: &mut Findings) {
    if let Some(root) = root(model)
        && is_machine(root)
    {
        findings.error(
            "L2R-S016",
            subject(root),
            "the root of a v2 model must be a composite/v2: it generates the scheduler and owns the system's ports",
        );
    }
}
