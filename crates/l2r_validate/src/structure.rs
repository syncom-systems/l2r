//! Structural rules: identities, containment, profiles and Rust mappings.

use std::collections::{BTreeMap, BTreeSet};

use l2r_model::{Component, Feature, Model, Profile, SUPPORTED_SCHEMAS, Schema, profiles};

use crate::diagnostic::Findings;
use crate::names;

/// Applies every structural rule of model schemas `v0` and `v1`.
pub fn check(model: &Model, findings: &mut Findings) {
    check_common(model, findings);
    for component in &model.components {
        check_profile_shape(model, component, findings);
        // The v2 profiles belong to model schema v2, which has its own rules.
        let later = component.parsed_profile().map(Profile::schema);
        if matches!(later, Some(Schema::V2)) {
            findings.error(
                "L2R-S019",
                subject(component),
                format!(
                    "profile `{}` needs `schema = \"l2r.architecture/v2\"`",
                    component.profile
                ),
            );
        }
    }
}

/// True when a `v0` document uses a feature or profile that `v1` introduced.
/// (The `v2` profiles and port fields have their own findings, S019 and the
/// port rules.)
fn uses_v1(model: &Model) -> bool {
    let v1_only = |f: &Feature| Schema::V1.admits_feature(*f) && !Schema::V0.admits_feature(*f);
    let v1_profile = |c: &Component| c.parsed_profile().map(Profile::schema) == Some(Schema::V1);
    model.features().iter().any(v1_only) || model.components.iter().any(v1_profile)
}

/// The rules every model schema shares: identities, containment and Rust mappings.
pub(crate) fn check_common(model: &Model, findings: &mut Findings) {
    check_schema(model, findings);
    check_system(model, findings);
    check_component_ids(model, findings);
    for component in &model.components {
        check_component_fields(component, findings);
    }
    check_parents(model, findings);
    check_single_root(model, findings);
    check_cycles(model, findings);
    check_sibling_modules(model, findings);
}

fn check_schema(model: &Model, findings: &mut Findings) {
    if model.parsed_schema().is_none() {
        findings.error(
            "L2R-S001",
            "schema",
            format!(
                "unsupported schema `{}`; this generator accepts {}",
                model.schema,
                SUPPORTED_SCHEMAS.join(", ")
            ),
        );
    }
    // A `v0` document means what it always meant: it cannot silently acquire
    // ports, capacities or the v1 profile.
    if model.parsed_schema() == Some(Schema::V0) && uses_v1(model) {
        findings.error(
            "L2R-S015",
            "schema",
            "ports, mailbox capacities and `composite/v1` need `schema = \"l2r.architecture/v1\"`",
        );
    }
}

fn check_system(model: &Model, findings: &mut Findings) {
    let system = &model.system;
    if !names::is_identifier(&system.id) {
        findings.error(
            "L2R-S002",
            "system.id",
            format!("`{}` is not a lower-snake-case identifier", system.id),
        );
    }
    if !names::is_identifier(&system.rust_crate) || names::is_rust_keyword(&system.rust_crate) {
        findings.error(
            "L2R-S002",
            "system.rust_crate",
            format!("`{}` is not a valid Rust crate name", system.rust_crate),
        );
    }
    check_paths(system, findings);
    check_text(&system.name, "system.name", findings);
}

/// Rule S013: every path in `[system]` is relative and cannot escape the
/// model's directory. The CLI applies it alone, through
/// [`crate::validate_system_paths`], before it opens the inventory these paths
/// name.
pub(crate) fn check_paths(system: &System, findings: &mut Findings) {
    for (field, path) in [
        ("system.lean_inventory", &system.lean_inventory),
        ("system.output_dir", &system.output_dir),
    ] {
        if !names::is_safe_relative_path(path) {
            findings.error(
                "L2R-S013",
                field,
                format!("`{path}` must be a relative path without `..` or hidden segments"),
            );
        }
    }
}

fn check_component_ids(model: &Model, findings: &mut Findings) {
    let mut seen = BTreeSet::new();
    for component in &model.components {
        if !seen.insert(component.id.as_str()) {
            findings.error("L2R-S003", subject(component), "duplicate component id");
        }
    }
}

fn check_component_fields(component: &Component, findings: &mut Findings) {
    let subject = subject(component);
    if !names::is_identifier(&component.id) {
        findings.error(
            "L2R-S002",
            &subject,
            "component id must be a lower-snake-case identifier",
        );
    }
    if !names::is_identifier(&component.rust_module) {
        findings.error(
            "L2R-S002",
            &subject,
            format!(
                "rust_module `{}` is not a valid module name",
                component.rust_module
            ),
        );
    } else if names::is_rust_keyword(&component.rust_module)
        || names::RESERVED_MODULES.contains(&component.rust_module.as_str())
    {
        findings.error(
            "L2R-S010",
            &subject,
            format!("rust_module `{}` is reserved", component.rust_module),
        );
    }
    if component.parsed_profile().is_none() {
        findings.error(
            "L2R-S007",
            &subject,
            format!(
                "unsupported profile `{}`; supported: {}",
                component.profile,
                profiles::SUPPORTED.join(", ")
            ),
        );
    }
    check_text(&component.name, &format!("{subject}.name"), findings);
    check_text(
        &component.responsibility,
        &format!("{subject}.responsibility"),
        findings,
    );
    if let Some(notes) = &component.notes
        && !names::is_clean_text(notes)
    {
        findings.error("L2R-S012", &subject, "notes contain control characters");
    }
    if component.implementation_budget_lines == Some(0) {
        findings.error(
            "L2R-S014",
            &subject,
            "implementation_budget_lines must be positive",
        );
    }
}

fn check_text(text: &str, subject: &str, findings: &mut Findings) {
    if text.trim().is_empty() {
        findings.error("L2R-S011", subject, "must not be empty");
    } else if !names::is_clean_text(text) {
        findings.error("L2R-S012", subject, "contains control characters");
    }
}

fn check_profile_shape(model: &Model, component: &Component, findings: &mut Findings) {
    let subject = subject(component);
    let has_children = model
        .components
        .iter()
        .any(|c| c.parent.as_deref() == Some(&component.id));
    match component.parsed_profile() {
        Some(Profile::TransitionCoreV0) => {
            if component.behavior.is_none() {
                findings.error(
                    "L2R-S008",
                    &subject,
                    "a transition-core component requires a [component.behavior] table",
                );
            }
            if has_children {
                findings.error(
                    "L2R-S008",
                    &subject,
                    "a transition-core component cannot contain other components",
                );
            }
        }
        Some(Profile::CompositeV0) => {
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
        }
        Some(Profile::CompositeV1) => {
            if component.behavior.is_some() {
                findings.error(
                    "L2R-S008",
                    &subject,
                    "a composite owns no behavioral state; remove [component.behavior]",
                );
            }
            if component.parent.is_some() {
                findings.error(
                    "L2R-S016",
                    &subject,
                    "composite/v1 must be the root component: other composites cannot drive its scheduler",
                );
            }
            let children: Vec<&Component> = model.children(&component.id);
            if children.is_empty() {
                findings.error(
                    "L2R-S008",
                    &subject,
                    "a composite must contain at least one component",
                );
            }
            for child in children {
                if !child
                    .parsed_profile()
                    .is_some_and(|p| Profile::CompositeV1.admits_child(p))
                {
                    findings.error(
                        "L2R-S016",
                        &subject,
                        format!(
                            "child `{}` must be a transition-core/v0 machine; composite/v1 does not nest composites",
                            child.id
                        ),
                    );
                }
            }
        }
        Some(Profile::TransitionCoreV1 | Profile::CompositeV2) | None => {}
    }
    check_capacity(model, component, findings);
}

/// Mailbox capacities belong to the machine children of `composite/v1`, and
/// each of those must state one: an unstated capacity would be a hidden choice.
fn check_capacity(model: &Model, component: &Component, findings: &mut Findings) {
    let in_v1 = component
        .parent
        .as_deref()
        .and_then(|p| model.component(p))
        .is_some_and(|p| p.parsed_profile() == Some(Profile::CompositeV1));
    let subject = subject(component);
    match (in_v1, component.mailbox_capacity) {
        (true, None) => findings.error(
            "L2R-S017",
            &subject,
            "a child of a composite/v1 must state `mailbox_capacity`",
        ),
        (true, Some(0)) => findings.error(
            "L2R-S017",
            &subject,
            "`mailbox_capacity` must be at least 1",
        ),
        (true, Some(n)) if n > MAX_MAILBOX_CAPACITY => findings.error(
            "L2R-S017",
            &subject,
            format!("`mailbox_capacity` {n} exceeds the supported maximum {MAX_MAILBOX_CAPACITY}"),
        ),
        (false, Some(_)) => findings.error(
            "L2R-S018",
            &subject,
            "`mailbox_capacity` applies only to the children of a composite/v1",
        ),
        _ => {}
    }
}

/// Largest mailbox the generator emits as a fixed-size array.
pub const MAX_MAILBOX_CAPACITY: u32 = 64;

fn check_parents(model: &Model, findings: &mut Findings) {
    for component in &model.components {
        if let Some(parent) = &component.parent
            && model.component(parent).is_none()
        {
            findings.error(
                "L2R-S004",
                subject(component),
                format!("parent `{parent}` does not exist"),
            );
        }
    }
}

fn check_single_root(model: &Model, findings: &mut Findings) {
    let roots: Vec<&str> = model
        .components
        .iter()
        .filter(|c| c.parent.is_none())
        .map(|c| c.id.as_str())
        .collect();
    if roots.len() != 1 {
        findings.error(
            "L2R-S006",
            "components",
            format!(
                "exactly one root component (without parent) is required; found {}: [{}]",
                roots.len(),
                roots.join(", ")
            ),
        );
    }
}

fn check_cycles(model: &Model, findings: &mut Findings) {
    let parent_of: BTreeMap<&str, &str> = model
        .components
        .iter()
        .filter_map(|c| c.parent.as_deref().map(|p| (c.id.as_str(), p)))
        .collect();
    for start in parent_of.keys() {
        let mut current = *start;
        for _ in 0..=parent_of.len() {
            match parent_of.get(current) {
                Some(&next) if next == *start => {
                    findings.error(
                        "L2R-S005",
                        format!("component `{start}`"),
                        "containment forms a cycle",
                    );
                    break;
                }
                Some(&next) => current = next,
                None => break,
            }
        }
    }
}

fn check_sibling_modules(model: &Model, findings: &mut Findings) {
    let mut seen: BTreeSet<(Option<&str>, &str)> = BTreeSet::new();
    for component in &model.components {
        if !seen.insert((component.parent.as_deref(), component.rust_module.as_str())) {
            findings.error(
                "L2R-S009",
                subject(component),
                format!(
                    "rust_module `{}` is already used by a sibling",
                    component.rust_module
                ),
            );
        }
    }
}

pub(crate) fn subject(component: &Component) -> String {
    format!("component `{}`", component.id)
}
