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
    let composite = Composite::of(generator, component);
    let mut artifacts = vec![composite.contract_artifact()?];
    if composite.owns_translations() {
        artifacts.push(composite.routes_artifact()?);
    }
    Ok(artifacts)
}

/// The two files of a composite's module.
#[derive(Clone, Copy)]
enum ModuleFile {
    /// `mod.rs`, generated.
    Contract,
    /// `routes.rs`, written by the implementer.
    Routes,
}

impl ModuleFile {
    fn name(self) -> &'static str {
        match self {
            ModuleFile::Contract => "mod.rs",
            ModuleFile::Routes => "routes.rs",
        }
    }

    fn ownership(self) -> Ownership {
        match self {
            ModuleFile::Contract => Ownership::Generated,
            ModuleFile::Routes => Ownership::Implementation,
        }
    }
}

/// A composite as its profile sees it: the component with its children, its
/// ports, and the translations it owns.
struct Composite<'a> {
    generator: &'a Gen<'a>,
    component: &'a Component,
    children: Vec<&'a Component>,
    ports: Vec<&'a Port>,
    /// The hops implemented in this composite's `routes.rs`.
    hops: Vec<&'a Hop>,
    /// The message edges among those hops.
    edges: Vec<&'a MessageEdge>,
}

impl<'a> Composite<'a> {
    fn of(generator: &'a Gen<'a>, component: &'a Component) -> Self {
        let model = generator.model;
        let hops = generator.hops_owned_by(&component.id);
        let edges = model
            .messages
            .iter()
            .filter(|e| hops.iter().any(|h| h.kind == HopKind::Edge && h.id == e.id))
            .collect();
        let ports = model
            .ports
            .iter()
            .filter(|p| p.composite == component.id)
            .collect();
        Self {
            generator,
            component,
            children: model.children(&component.id),
            ports,
            hops,
            edges,
        }
    }

    fn is_root(&self) -> bool {
        self.component.parent.is_none()
    }

    fn owns_translations(&self) -> bool {
        !self.hops.is_empty()
    }

    /// The directory of the composite's module.
    fn dir(&self) -> &str {
        &self.generator.dirs[&self.component.id]
    }

    /// The Lean declarations the composite's files depend on.
    fn lean(&self) -> Vec<String> {
        let mut lean: Vec<String> = self.hops.iter().map(|h| h.translation.clone()).collect();
        lean.extend(self.ports.iter().map(|p| p.message.clone()));
        lean.sort();
        lean.dedup();
        lean
    }

    /// The components the composite's files are generated from: itself first,
    /// then the machines it schedules (the root) or its children (a nested one).
    fn sources(&self) -> Vec<String> {
        let id = &self.component.id;
        let mut sources: Vec<String> = if self.is_root() {
            let machines = &self.generator.flat.leaves;
            machines.iter().map(|l| l.id.clone()).collect()
        } else {
            self.children.iter().map(|c| c.id.clone()).collect()
        };
        sources.sort();
        sources.dedup();
        sources.retain(|s| s != id);
        sources.insert(0, id.clone());
        sources
    }

    /// `mod.rs`: the contract, generated.
    fn contract_artifact(&self) -> Result<Artifact, PlanError> {
        self.artifact(ModuleFile::Contract, self.contract()?)
    }

    /// `routes.rs`: the translations, for the implementer.
    fn routes_artifact(&self) -> Result<Artifact, PlanError> {
        self.artifact(ModuleFile::Routes, self.routes()?)
    }

    /// What a file of this composite's module is for.
    fn purpose(&self, file: ModuleFile) -> &'static str {
        match file {
            ModuleFile::Contract if self.is_root() => {
                "Root composite: boundary, bounded mailboxes and outboxes, and the scheduler"
            }
            ModuleFile::Contract => "Composite contract: children and typed boundary",
            ModuleFile::Routes => "Message and port translations, owned by the implementer",
        }
    }

    /// A file of the composite's module, with what it was generated from.
    fn artifact(&self, file: ModuleFile, content: String) -> Result<Artifact, PlanError> {
        Ok(Artifact {
            path: planned_path(format!("{}/{}", self.dir(), file.name()))?,
            ownership: file.ownership(),
            profile: COMPOSITE_V2.into(),
            purpose: self.purpose(file).into(),
            components: self.sources(),
            lean: self.lean(),
            content,
        })
    }

    /// The text of `mod.rs`: the documented boundary, the modules and, for the
    /// root, the `System`.
    fn contract(&self) -> Result<String, PlanError> {
        let mut contract = text::generated_marker(COMPOSITE_V2);
        contract.push('\n');
        contract.push_str(&text::inner_doc(&self.module_doc()));
        contract.push('\n');
        contract.push_str(&self.module_declarations());
        if self.is_root() {
            contract.push('\n');
            contract.push_str(&system::system(self.generator)?);
        }
        Ok(contract)
    }

    /// One `mod` per child, and `routes` when the composite owns a translation.
    fn module_declarations(&self) -> String {
        let mut modules: Vec<(String, &str)> = self
            .children
            .iter()
            .map(|c| (c.rust_module.clone(), "pub "))
            .collect();
        if self.owns_translations() {
            modules.push(("routes".into(), "pub(crate) "));
        }
        modules.sort();
        modules
            .iter()
            .map(|(module, visibility)| format!("{visibility}mod {module};\n"))
            .collect()
    }

    fn module_doc(&self) -> String {
        [
            self.summary_doc(),
            self.boundary_doc(),
            self.composition_doc(),
            self.behaviour_doc(),
        ]
        .concat()
    }

    /// What the composite is and is responsible for.
    fn summary_doc(&self) -> String {
        let component = self.component;
        let mut doc = format!(
            "{} (`{}`), profile `composite/v2`, semantics `l2r.async/v1`.\n\nResponsibility: {}\n",
            component.name, component.id, component.responsibility
        );
        if let Some(notes) = &component.notes {
            doc.push_str(&format!("\nArchitect's notes: {notes}\n"));
        }
        doc
    }

    /// `# Boundary`: the ports.
    fn boundary_doc(&self) -> String {
        let ports: Vec<String> = self.ports.iter().map(|port| port_doc(port)).collect();
        [
            "\n# Boundary\n\nPorts (the only way a message crosses this composite's boundary):\n\n",
            &listed(&ports),
        ]
        .concat()
    }

    /// `# Composition`: the children and the message edges.
    fn composition_doc(&self) -> String {
        let children: String = self.children.iter().map(|child| child_doc(child)).collect();
        let edges: Vec<String> = self.edges.iter().map(|edge| edge_doc(edge)).collect();
        [
            "\n# Composition\n\nChildren:\n\n",
            &children,
            "\nApproved message edges in this composite:\n\n",
            &listed(&edges),
        ]
        .concat()
    }

    /// `# Behaviour`: the flat network for the root; for a nested composite,
    /// that it has no behaviour of its own.
    fn behaviour_doc(&self) -> String {
        if self.is_root() {
            return system::behaviour_doc(self.generator);
        }
        format!(
            "\n# Behaviour\n\n\
             A nested composite is structure: it owns no state and runs no scheduler.\n\
             Its machines are scheduled by the root composite `{}`, as part of the\n\
             flat network of every machine in the system. What this module fixes is\n\
             the boundary: nothing reaches its children, and nothing leaves them,\n\
             except through the ports above.\n",
            self.generator.root.id
        )
    }

    /// The text of `routes.rs`: one stub per translation the composite owns.
    fn routes(&self) -> Result<String, PlanError> {
        let component = self.component;
        let mut out = text::inner_doc(&format!(
            "Message and port translations for {} (`{}`).\n\
             \n\
             This file belongs to the implementer. L2R created it once and will never\n\
             overwrite it. Each function must agree with the Lean translation it names.\n\
             An edge translation may return `None`: the message then means nothing to\n\
             the receiver. A port translation is total.\n",
            component.name, component.id
        ));
        for hop in &self.hops {
            out.push_str(&translation_stub(self.generator, hop)?);
        }
        Ok(out)
    }
}

/// The items of a documented list, or `- none`.
fn listed(items: &[String]) -> String {
    if items.is_empty() {
        return "- none\n".into();
    }
    items.concat()
}

/// A port, as the boundary documentation lists it.
fn port_doc(port: &Port) -> String {
    if port.is_out() {
        let capacity = port
            .capacity
            .map(|n| format!(" (outbox capacity {n})"))
            .unwrap_or_default();
        return format!("- out `{}`: `{}`{capacity}\n", port.id, port.message);
    }
    format!(
        "- in `{}` → `{}`: `{}` through `{}`\n",
        port.id,
        port.target(),
        port.message,
        port.translation_name()
    )
}

/// A child, as the composition documentation lists it: a machine has a mailbox.
fn child_doc(child: &Component) -> String {
    match child.mailbox_capacity {
        Some(n) => format!(
            "- `{}`: {} (machine, mailbox capacity {n})\n",
            child.id, child.name
        ),
        None => format!("- `{}`: {} (composite)\n", child.id, child.name),
    }
}

/// A message edge, as the composition documentation lists it.
fn edge_doc(edge: &MessageEdge) -> String {
    format!(
        "- `{}`: `{}` → `{}`, meaning `{}`\n",
        edge.id, edge.from, edge.to, edge.translation
    )
}

/// The stub of one translation: an edge may refuse a message, a port is total.
fn translation_stub(generator: &Gen<'_>, hop: &Hop) -> Result<String, PlanError> {
    let source = generator.type_path(&hop_source(generator, hop)?)?;
    let result = generator.type_path(&hop.result)?;
    let name = hop_function_name(hop);
    let (what, returns) = match hop.kind {
        HopKind::Edge => (format!("edge `{}`", hop.id), format!("Option<{result}>")),
        HopKind::Port => (format!("port `{}`", hop.id), result),
    };
    Ok(format!(
        "\n/// Implements `{t}` ({what}).\n\
         pub(crate) fn {name}(\n    message: {source},\n) -> {returns} {{\n    \
             let _ = message;\n    \
             todo!(\"implement {t}\")\n}}\n",
        t = hop.translation,
    ))
}

/// The Lean type a hop translates from.
fn hop_source(generator: &Gen<'_>, hop: &Hop) -> Result<String, PlanError> {
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
