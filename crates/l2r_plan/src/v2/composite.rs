//! Profile `composite/v2`: a composite that may be nested and has typed ports.
//!
//! Every composite generates `mod.rs` (its children, and its documented
//! boundary) and, when it owns any translation, `routes.rs` for the implementer:
//! one function per message edge that lives in it and per `in` port it has.
//! The root composite's `mod.rs` additionally holds the `System` (see
//! [`super::system`]).

use l2r_model::{Component, MessageEdge, Port, profiles::COMPOSITE_V2};
use l2r_validate::flat::{Hop, HopKind};

use crate::plan::{Artifact, Ownership};
use crate::text;
use crate::v2::{Gen, hop_function_name, system};
use crate::{PlanError, planned_path};

pub fn artifacts(generator: &Gen<'_>, component: &Component) -> Result<Vec<Artifact>, PlanError> {
    let model = generator.model;
    let dir = &generator.dirs[&component.id];
    let children = model.children(&component.id);
    let ports: Vec<&Port> = model
        .ports
        .iter()
        .filter(|p| p.composite == component.id)
        .collect();
    let hops = generator.hops_owned_by(&component.id);
    let edges: Vec<&MessageEdge> = model
        .messages
        .iter()
        .filter(|e| hops.iter().any(|h| h.kind == HopKind::Edge && h.id == e.id))
        .collect();
    let is_root = component.parent.is_none();

    let mut lean: Vec<String> = hops.iter().map(|h| h.translation.clone()).collect();
    lean.extend(ports.iter().map(|p| p.message.clone()));
    lean.sort();
    lean.dedup();
    let mut sources = vec![component.id.clone()];
    if is_root {
        sources.extend(generator.flat.leaves.iter().map(|l| l.id.clone()));
    } else {
        sources.extend(children.iter().map(|c| c.id.clone()));
    }
    sources.sort();
    sources.dedup();
    // The owning component comes first.
    sources.retain(|s| *s != component.id);
    sources.insert(0, component.id.clone());

    let mut contract = text::generated_marker(COMPOSITE_V2);
    contract.push('\n');
    contract.push_str(&text::inner_doc(&module_doc(
        generator, component, &children, &ports, &edges, is_root,
    )));
    contract.push('\n');
    let mut modules: Vec<(String, &str)> = children
        .iter()
        .map(|c| (c.rust_module.clone(), "pub "))
        .collect();
    if !hops.is_empty() {
        modules.push(("routes".into(), "pub(crate) "));
    }
    modules.sort();
    for (module, visibility) in modules {
        contract.push_str(&format!("{visibility}mod {module};\n"));
    }
    if is_root {
        contract.push('\n');
        contract.push_str(&system::system(generator)?);
    }

    let mut artifacts = vec![Artifact {
        path: planned_path(format!("{dir}/mod.rs"))?,
        ownership: Ownership::Generated,
        profile: COMPOSITE_V2.into(),
        purpose: if is_root {
            "Root composite: boundary, bounded mailboxes and outboxes, and the scheduler".into()
        } else {
            "Composite contract: children and typed boundary".into()
        },
        components: sources.clone(),
        lean: lean.clone(),
        content: contract,
    }];
    if !hops.is_empty() {
        artifacts.push(Artifact {
            path: planned_path(format!("{dir}/routes.rs"))?,
            ownership: Ownership::Implementation,
            profile: COMPOSITE_V2.into(),
            purpose: "Message and port translations, owned by the implementer".into(),
            components: sources,
            lean,
            content: routes(generator, component, &hops)?,
        });
    }
    Ok(artifacts)
}

fn module_doc(
    generator: &Gen<'_>,
    component: &Component,
    children: &[&Component],
    ports: &[&Port],
    edges: &[&MessageEdge],
    is_root: bool,
) -> String {
    let mut doc = format!(
        "{} (`{}`), profile `composite/v2`, semantics `l2r.async/v1`.\n\nResponsibility: {}\n",
        component.name, component.id, component.responsibility
    );
    if let Some(notes) = &component.notes {
        doc.push_str(&format!("\nArchitect's notes: {notes}\n"));
    }
    doc.push_str(
        "\n# Boundary\n\nPorts (the only way a message crosses this composite's boundary):\n\n",
    );
    if ports.is_empty() {
        doc.push_str("- none\n");
    }
    for port in ports {
        if port.is_out() {
            let capacity = port
                .capacity
                .map(|n| format!(" (outbox capacity {n})"))
                .unwrap_or_default();
            doc.push_str(&format!(
                "- out `{}`: `{}`{capacity}\n",
                port.id, port.message
            ));
        } else {
            doc.push_str(&format!(
                "- in `{}` → `{}`: `{}` through `{}`\n",
                port.id,
                port.target(),
                port.message,
                port.translation_name()
            ));
        }
    }
    doc.push_str("\n# Composition\n\nChildren:\n\n");
    for child in children {
        match child.mailbox_capacity {
            Some(n) => doc.push_str(&format!(
                "- `{}`: {} (machine, mailbox capacity {n})\n",
                child.id, child.name
            )),
            None => doc.push_str(&format!("- `{}`: {} (composite)\n", child.id, child.name)),
        }
    }
    doc.push_str("\nApproved message edges in this composite:\n\n");
    if edges.is_empty() {
        doc.push_str("- none\n");
    }
    for edge in edges {
        doc.push_str(&format!(
            "- `{}`: `{}` → `{}`, meaning `{}`\n",
            edge.id, edge.from, edge.to, edge.translation
        ));
    }
    if is_root {
        doc.push_str(&system::behaviour_doc(generator));
    } else {
        doc.push_str(&format!(
            "\n# Behaviour\n\n\
             A nested composite is structure: it owns no state and runs no scheduler.\n\
             Its machines are scheduled by the root composite `{}`, as part of the\n\
             flat network of every machine in the system. What this module fixes is\n\
             the boundary: nothing reaches its children, and nothing leaves them,\n\
             except through the ports above.\n",
            generator.root.id
        ));
    }
    doc
}

fn routes(generator: &Gen<'_>, component: &Component, hops: &[&Hop]) -> Result<String, PlanError> {
    let mut out = text::inner_doc(&format!(
        "Message and port translations for {} (`{}`).\n\
         \n\
         This file belongs to the implementer. L2R created it once and will never\n\
         overwrite it. Each function must agree with the Lean translation it names.\n\
         An edge translation may return `None`: the message then means nothing to\n\
         the receiver. A port translation is total.\n",
        component.name, component.id
    ));
    for hop in hops {
        let source = generator.type_path(&hop_source(generator, hop)?)?;
        let result = generator.type_path(&hop.result)?;
        let name = hop_function_name(hop);
        let (what, returns) = match hop.kind {
            HopKind::Edge => (format!("edge `{}`", hop.id), format!("Option<{result}>")),
            HopKind::Port => (format!("port `{}`", hop.id), result),
        };
        out.push_str(&format!(
            "\n/// Implements `{t}` ({what}).\n\
             pub(crate) fn {name}(\n    message: {source},\n) -> {returns} {{\n    \
                 let _ = message;\n    \
                 todo!(\"implement {t}\")\n}}\n",
            t = hop.translation,
        ));
    }
    Ok(out)
}

/// The Lean type a hop translates from.
pub(crate) fn hop_source(generator: &Gen<'_>, hop: &Hop) -> Result<String, PlanError> {
    let model = generator.model;
    let missing = || PlanError(format!("hop `{}` has no source type", hop.id));
    match hop.kind {
        HopKind::Port => model
            .ports
            .iter()
            .find(|p| p.id == hop.id)
            .map(|p| p.message.clone())
            .ok_or_else(missing),
        HopKind::Edge => {
            let edge = model
                .messages
                .iter()
                .find(|e| e.id == hop.id)
                .ok_or_else(missing)?;
            if let Some(port) = model.ports.iter().find(|p| p.id == edge.from) {
                return Ok(port.message.clone());
            }
            model
                .component(&edge.from)
                .and_then(|c| c.behavior.as_ref())
                .map(|b| b.output.clone())
                .ok_or_else(missing)
        }
    }
}
