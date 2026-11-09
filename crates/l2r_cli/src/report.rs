//! Human-readable rendering of plans, previews and architecture views.

use std::fmt::Write as _;

use l2r_apply::{ActionKind, Assessment};
use l2r_model::{Model, profiles};
use l2r_plan::{Ownership, Plan};

use crate::diff::line_diff;

fn ownership(o: Ownership) -> &'static str {
    match o {
        Ownership::Generated => "generated",
        Ownership::Implementation => "implementation",
    }
}

/// The plan summary: inputs, every planned file, and the digest to approve.
pub fn plan_summary(model: &Model, plan: &Plan) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Plan for system `{}` ({} {})",
        model.system.id, plan.generator.name, plan.generator.version
    );
    let _ = writeln!(out, "  model digest      {}", plan.model_digest);
    let _ = writeln!(out, "  inventory digest  {}", plan.inventory_digest);
    let _ = writeln!(out, "  profiles          {}", plan.profiles.join(", "));
    let _ = writeln!(out, "  output directory  {}/", plan.output_dir);
    let _ = writeln!(out, "\nPlanned files ({}):", plan.artifacts.len());
    for a in &plan.artifacts {
        let _ = writeln!(
            out,
            "  {:<15} {:<46} {}",
            ownership(a.ownership),
            a.path,
            a.purpose
        );
        let _ = writeln!(
            out,
            "  {:<15} {:<46} from [{}]",
            "",
            "",
            a.components.join(", ")
        );
    }
    let _ = writeln!(out, "\nPlan digest: {}", plan.digest());
    out
}

fn action_label(kind: ActionKind) -> &'static str {
    match kind {
        ActionKind::Create => "create",
        ActionKind::Update => "update",
        ActionKind::Unchanged => "unchanged",
        ActionKind::Keep => "keep (implementation exists)",
        ActionKind::Delete => "delete (no longer planned)",
        ActionKind::Orphan => "orphan (left in place)",
    }
}

/// The preview: the plan summary, what would happen on disk, and exact content
/// or diffs for every file that would change.
pub fn preview(model: &Model, plan: &Plan, assessment: &Assessment, show_content: bool) -> String {
    let mut out = plan_summary(model, plan);
    let _ = writeln!(out, "\nEffect on {}/:", plan.output_dir);
    for action in &assessment.actions {
        let _ = writeln!(out, "  {:<29} {}", action_label(action.kind), action.path);
    }
    let destructive = l2r_apply::destructive_changes(assessment);
    if !destructive.is_empty() {
        let _ = writeln!(
            out,
            "\nApproving records permission to destroy existing content ({}):",
            destructive.len()
        );
        for c in &destructive {
            let _ = writeln!(out, "  {:?} {} while it is {}", c.change, c.path, c.before);
        }
    }
    if !assessment.conflicts.is_empty() {
        let _ = writeln!(
            out,
            "\nConflicts ({}) — generate will refuse to write anything:",
            assessment.conflicts.len()
        );
        for c in &assessment.conflicts {
            let _ = writeln!(out, "  {}: {}", c.path, c.reason);
        }
    }
    if show_content {
        for action in &assessment.actions {
            let planned = plan
                .artifact(action.path.as_str())
                .map(|a| a.content.as_str());
            let current = action.current.as_deref().map(String::from_utf8_lossy);
            let body = match (action.kind, planned, current) {
                (ActionKind::Create, Some(new), _) => line_diff("", new),
                (ActionKind::Update, Some(new), Some(old)) => line_diff(&old, new),
                (ActionKind::Delete, _, Some(old)) => line_diff(&old, ""),
                _ => continue,
            };
            let _ = writeln!(
                out,
                "\n=== {} ({}) ===\n{body}",
                action.path,
                action_label(action.kind)
            );
        }
    }
    let _ = writeln!(
        out,
        "\nTo approve exactly this plan:\n  l2r approve <model> --digest {} --by <your name>",
        plan.digest()
    );
    out
}

fn quoted(text: &str) -> String {
    text.replace('"', "#quot;")
}

/// A Mermaid view of the architecture: containment as nested subgraphs, state
/// ownership on each machine, approved message edges as arrows.
pub fn mermaid(model: &Model) -> String {
    if model.schema == l2r_model::MODEL_SCHEMA_V2 {
        return mermaid_v2(model);
    }
    let mut out = String::from("flowchart TB\n");
    if let Some(root) = model.components.iter().find(|c| c.parent.is_none()) {
        render_node(model, &root.id, 1, &mut out);
    }
    for edge in &model.messages {
        let _ = writeln!(
            out,
            "  {} -- \"{}\" --> {}",
            edge.from,
            quoted(&edge.id),
            edge.to
        );
    }
    // Typed input ports: the only way the environment reaches a composite/v1.
    for port in &model.ports {
        let _ = writeln!(
            out,
            "  env_{id}([\"port {id}: {msg}\"]) -- \"{id}\" --> {to}",
            id = port.id,
            msg = quoted(&port.message),
            to = port.target()
        );
    }
    out
}

/// The view of a hierarchical (`v2`) model. Ports are drawn as nodes on the
/// boundary of their composite, because message edges name them: an `in` port
/// points at what it feeds, and edges enter and leave composites only through
/// ports.
fn mermaid_v2(model: &Model) -> String {
    let mut out = String::from("flowchart TB\n");
    if let Some(root) = model.components.iter().find(|c| c.parent.is_none()) {
        render_node(model, &root.id, 1, &mut out);
    }
    let node = |id: &str| {
        if model.ports.iter().any(|p| p.id == id) {
            format!("port_{id}")
        } else {
            id.to_string()
        }
    };
    for port in model.ports.iter().filter(|p| !p.is_out()) {
        let _ = writeln!(
            out,
            "  port_{} -- \"{}\" --> {}",
            port.id,
            quoted(port.translation_name()),
            node(port.target())
        );
    }
    for edge in &model.messages {
        let _ = writeln!(
            out,
            "  {} -- \"{}\" --> {}",
            node(&edge.from),
            quoted(&edge.id),
            node(&edge.to)
        );
    }
    out
}

fn render_node(model: &Model, id: &str, depth: usize, out: &mut String) {
    let Some(component) = model.component(id) else {
        return;
    };
    let indent = "  ".repeat(depth);
    if [
        profiles::COMPOSITE_V0,
        profiles::COMPOSITE_V1,
        profiles::COMPOSITE_V2,
    ]
    .contains(&component.profile.as_str())
    {
        let _ = writeln!(
            out,
            "{indent}subgraph {}[\"{} ({})\"]",
            component.id,
            quoted(&component.name),
            component.profile
        );
        if component.profile == profiles::COMPOSITE_V2 {
            for port in model.ports.iter().filter(|p| p.composite == component.id) {
                let (shape_open, shape_close, direction) = if port.is_out() {
                    ("[/\"", "\"/]", "out")
                } else {
                    ("[\\\"", "\"\\]", "in")
                };
                let capacity = port
                    .capacity
                    .map(|n| format!("<br/>outbox {n}"))
                    .unwrap_or_default();
                let _ = writeln!(
                    out,
                    "{indent}  port_{id}{shape_open}{direction} {id}<br/>{msg}{capacity}{shape_close}",
                    id = port.id,
                    msg = quoted(&port.message),
                );
            }
        }
        for child in model.children(id) {
            render_node(model, &child.id, depth + 1, out);
        }
        let _ = writeln!(out, "{indent}end");
    } else {
        let owns = component
            .behavior
            .as_ref()
            .map(|b| b.owned_state.as_str())
            .unwrap_or("-");
        let capacity = component
            .mailbox_capacity
            .map(|n| format!("<br/>mailbox {n}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "{indent}{}[\"{}<br/>owns {}<br/>{}{capacity}\"]",
            component.id,
            quoted(&component.name),
            quoted(owns),
            component.profile
        );
    }
}
