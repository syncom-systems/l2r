//! Profile `composite/v0`: deterministic asynchronous composition of children.
//!
//! Generates `mod.rs` (generated) declaring the children and a `System` that
//! holds one owned state and one FIFO mailbox per machine child (and the nested
//! `System` of each composite child). When the composite has message edges it
//! also plans `routes.rs` (implementation), holding one translation function per
//! edge for the implementer to write.

use l2r_model::rust_names::composite_v0::SystemNames;
use l2r_model::{Component, MessageEdge, Model, Profile, profiles};

use crate::plan::{Artifact, Ownership};
use crate::text;
use crate::{PlanError, planned_path};

/// Plans the files for one composite component in directory `dir`, with the
/// generated names validation checked for it.
pub fn artifacts(
    model: &Model,
    component: &Component,
    names: &SystemNames,
    dir: &str,
) -> Result<Vec<Artifact>, PlanError> {
    let children = model.children(&component.id);
    let edges = model.edges_within(&component.id);
    let mut lean: Vec<String> = edges.iter().map(|e| e.translation.clone()).collect();
    lean.sort();
    let mut sources = vec![component.id.clone()];
    sources.extend(children.iter().map(|c| c.id.clone()));
    let mut artifacts = vec![Artifact {
        path: planned_path(format!("{dir}/mod.rs"))?,
        ownership: Ownership::Generated,
        profile: profiles::COMPOSITE_V0.into(),
        purpose: "Composite contract: children, mailboxes and deterministic scheduler".into(),
        components: sources.clone(),
        lean: lean.clone(),
        content: contract(component, names, &children, &edges),
    }];
    if !edges.is_empty() {
        artifacts.push(Artifact {
            path: planned_path(format!("{dir}/routes.rs"))?,
            ownership: Ownership::Implementation,
            profile: profiles::COMPOSITE_V0.into(),
            purpose: "Message translations, owned by the implementer".into(),
            components: sources,
            lean,
            content: routes(model, component, names, &edges),
        });
    }
    Ok(artifacts)
}

pub(crate) fn is_machine(c: &Component) -> bool {
    c.parsed_profile() == Some(Profile::TransitionCoreV0)
}

fn contract(
    component: &Component,
    names: &SystemNames,
    children: &[&Component],
    edges: &[&MessageEdge],
) -> String {
    let mut out = text::generated_marker(profiles::COMPOSITE_V0);
    out.push('\n');
    out.push_str(&text::inner_doc(&module_doc(component, children, edges)));
    out.push('\n');
    // One group, sorted by module name, as rustfmt orders module declarations.
    let mut modules: Vec<(String, &str)> = children
        .iter()
        .map(|c| (c.rust_module.clone(), "pub "))
        .collect();
    if !edges.is_empty() {
        modules.push(("routes".into(), ""));
    }
    modules.sort();
    for (module, visibility) in modules {
        out.push_str(&format!("{visibility}mod {module};\n"));
    }
    out.push_str("\nuse std::collections::VecDeque;\n\n");
    out.push_str(&envelope(names, children));
    out.push_str(&system_struct(names, children));
    out.push_str(&unsettled());
    out.push_str(&system_impl(names, children, edges));
    out.push_str(
        "impl Default for System {\n    fn default() -> Self {\n        Self::new()\n    }\n}\n",
    );
    out
}

fn module_doc(component: &Component, children: &[&Component], edges: &[&MessageEdge]) -> String {
    let mut doc = format!(
        "{} (`{}`).\n\nResponsibility: {}\n",
        component.name, component.id, component.responsibility
    );
    if let Some(notes) = &component.notes {
        doc.push_str(&format!("\nArchitect's notes: {notes}\n"));
    }
    doc.push_str("\n# Composition\n\nChildren:\n\n");
    for child in children {
        let kind = if is_machine(child) {
            "machine"
        } else {
            "composite"
        };
        doc.push_str(&format!("- `{}` ({kind}): {}\n", child.id, child.name));
    }
    doc.push_str("\nApproved message edges:\n\n");
    if edges.is_empty() {
        doc.push_str("- none\n");
    }
    for edge in edges {
        doc.push_str(&format!(
            "- `{}`: `{}` → `{}`, meaning `{}`\n",
            edge.id, edge.from, edge.to, edge.translation
        ));
    }
    doc.push_str(
        "\nEach machine's state is owned by that machine and changed only by its own\n\
         `step`. Messages travel through per-child FIFO mailboxes; `System::step_once`\n\
         serves children round-robin in identifier order. This is one deterministic\n\
         scheduler, not a model of every interleaving the Lean semantics may permit.\n\
         Outputs for which no edge is approved are discarded.\n",
    );
    doc
}

fn envelope(names: &SystemNames, children: &[&Component]) -> String {
    let mut out = String::from(
        "/// A message addressed to one child.\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Envelope {\n",
    );
    for child in children {
        let payload = if is_machine(child) {
            "Input"
        } else {
            "Envelope"
        };
        out.push_str(&format!(
            "    /// For `{}`.\n    {}({}::{payload}),\n",
            child.id,
            names.variant(&child.id),
            child.rust_module
        ));
    }
    out.push_str("}\n\n");
    out
}

fn system_struct(names: &SystemNames, children: &[&Component]) -> String {
    let mut out = String::from(
        "/// The composed system: one owned state and mailbox per machine child, and the\n\
         /// nested system of each composite child.\n\
         #[derive(Debug, Clone, PartialEq, Eq)]\npub struct System {\n",
    );
    for child in children {
        let m = &child.rust_module;
        let field = names.field(&child.id);
        if is_machine(child) {
            let inbox = names.inbox(&child.id);
            out.push_str(&format!(
                "    {field}: {m}::State,\n    {inbox}: VecDeque<{m}::Input>,\n"
            ));
        } else {
            out.push_str(&format!("    {field}: {m}::System,\n"));
        }
    }
    out.push_str("    next: usize,\n}\n\n");
    out
}

fn unsettled() -> String {
    "/// `System::run` stopped with messages still pending.\n\
     #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
     pub struct Unsettled {\n    /// Deliveries performed before stopping.\n    pub steps: usize,\n}\n\n"
        .into()
}

fn system_impl(names: &SystemNames, children: &[&Component], edges: &[&MessageEdge]) -> String {
    let mut out = format!(
        "impl System {{\n    const CHILDREN: usize = {};\n\n",
        children.len()
    );
    out.push_str(&constructor(names, children));
    out.push_str(&accessors(names, children));
    out.push_str(&send(names, children));
    out.push_str(&is_settled(names, children));
    out.push_str(STEP_ONCE_AND_RUN);
    out.push_str(&deliver(names, children));
    for child in children.iter().filter(|c| is_machine(c)) {
        out.push_str(&route_from(names, child, edges));
    }
    // Every method above ends with a blank line; the last one must not.
    out.truncate(out.trim_end().len());
    out.push_str("\n}\n\n");
    out
}

fn constructor(names: &SystemNames, children: &[&Component]) -> String {
    let mut out = String::from(
        "    /// Every child in its initial state, with empty mailboxes.\n    pub fn new() -> Self {\n        Self {\n",
    );
    for child in children {
        let m = &child.rust_module;
        let field = names.field(&child.id);
        if is_machine(child) {
            let inbox = names.inbox(&child.id);
            out.push_str(&format!(
                "            {field}: {m}::init(),\n            {inbox}: VecDeque::new(),\n"
            ));
        } else {
            out.push_str(&format!("            {field}: {m}::System::new(),\n"));
        }
    }
    out.push_str("            next: 0,\n        }\n    }\n\n");
    out
}

fn accessors(names: &SystemNames, children: &[&Component]) -> String {
    let mut out = String::new();
    for child in children {
        let m = &child.rust_module;
        let field = names.field(&child.id);
        if is_machine(child) {
            out.push_str(&format!(
                "    /// Current state of `{}`. Read-only: only `{m}::step` changes it.\n    pub fn {field}(&self) -> {m}::State {{\n        self.{field}\n    }}\n\n",
                child.id
            ));
        } else {
            out.push_str(&format!(
                "    /// The nested system `{}`. Read-only.\n    pub fn {field}(&self) -> &{m}::System {{\n        &self.{field}\n    }}\n\n",
                child.id
            ));
        }
    }
    out
}

fn send(names: &SystemNames, children: &[&Component]) -> String {
    let mut out = String::from(
        "    /// Queues a message for its addressee.\n    pub fn send(&mut self, envelope: Envelope) {\n        match envelope {\n",
    );
    for child in children {
        let variant = names.variant(&child.id);
        if is_machine(child) {
            let inbox = names.inbox(&child.id);
            out.push_str(&format!(
                "            Envelope::{variant}(input) => self.{inbox}.push_back(input),\n"
            ));
        } else {
            let field = names.field(&child.id);
            out.push_str(&format!(
                "            Envelope::{variant}(inner) => self.{field}.send(inner),\n"
            ));
        }
    }
    out.push_str("        }\n    }\n\n");
    out
}

fn is_settled(names: &SystemNames, children: &[&Component]) -> String {
    let checks: Vec<String> = children
        .iter()
        .map(|c| {
            if is_machine(c) {
                format!("self.{}.is_empty()", names.inbox(&c.id))
            } else {
                format!("self.{}.is_settled()", names.field(&c.id))
            }
        })
        .collect();
    // One short statement per child keeps the layout independent of line width.
    let mut body = String::from("        let mut settled = true;\n");
    for check in checks {
        body.push_str(&format!("        settled &= {check};\n"));
    }
    format!(
        "    /// True when no mailbox holds a message.\n    pub fn is_settled(&self) -> bool {{\n{body}        settled\n    }}\n\n"
    )
}

const STEP_ONCE_AND_RUN: &str = "    /// Serves the next child, in round-robin order, that has pending work.\n\
     \x20   /// Returns `false` when the system is settled.\n\
     \x20   pub fn step_once(&mut self) -> bool {\n\
     \x20       for offset in 0..Self::CHILDREN {\n\
     \x20           let child = (self.next + offset) % Self::CHILDREN;\n\
     \x20           if self.deliver(child) {\n\
     \x20               self.next = (child + 1) % Self::CHILDREN;\n\
     \x20               return true;\n\
     \x20           }\n\
     \x20       }\n\
     \x20       false\n\
     \x20   }\n\n\
     \x20   /// Delivers messages until settled or until `max_steps` deliveries.\n\
     \x20   pub fn run(&mut self, max_steps: usize) -> Result<usize, Unsettled> {\n\
     \x20       for steps in 0..max_steps {\n\
     \x20           if !self.step_once() {\n\
     \x20               return Ok(steps);\n\
     \x20           }\n\
     \x20       }\n\
     \x20       if self.is_settled() {\n\
     \x20           Ok(max_steps)\n\
     \x20       } else {\n\
     \x20           Err(Unsettled { steps: max_steps })\n\
     \x20       }\n\
     \x20   }\n\n";

fn deliver(names: &SystemNames, children: &[&Component]) -> String {
    let mut out =
        String::from("    fn deliver(&mut self, child: usize) -> bool {\n        match child {\n");
    for (index, child) in children.iter().enumerate() {
        let m = &child.rust_module;
        let field = names.field(&child.id);
        if is_machine(child) {
            let inbox = names.inbox(&child.id);
            let route_from = names.route_from(&child.id);
            out.push_str(&format!(
                "            {index} => {{\n\
                 \x20               let Some(input) = self.{inbox}.pop_front() else {{\n\
                 \x20                   return false;\n\
                 \x20               }};\n\
                 \x20               let (state, output) = {m}::step(self.{field}, input);\n\
                 \x20               self.{field} = state;\n\
                 \x20               if let Some(output) = output {{\n\
                 \x20                   self.{route_from}(output);\n\
                 \x20               }}\n\
                 \x20               true\n\
                 \x20           }}\n"
            ));
        } else {
            out.push_str(&format!(
                "            {index} => self.{field}.step_once(),\n"
            ));
        }
    }
    out.push_str("            _ => false,\n        }\n    }\n\n");
    out
}

fn route_from(names: &SystemNames, child: &Component, edges: &[&MessageEdge]) -> String {
    let m = &child.rust_module;
    let outgoing: Vec<&&MessageEdge> = edges.iter().filter(|e| e.from == child.id).collect();
    let route_from = names.route_from(&child.id);
    let mut out = format!("    fn {route_from}(&mut self, output: {m}::Output) {{\n");
    if outgoing.is_empty() {
        out.push_str(
            "        // No approved edge consumes these outputs.\n        let _ = output;\n",
        );
    }
    for edge in outgoing {
        out.push_str(&format!(
            "        if let Some(input) = routes::{}(output) {{\n            self.{}.push_back(input);\n        }}\n",
            names.edge_function(&edge.id),
            names.inbox(&edge.to)
        ));
    }
    out.push_str("    }\n\n");
    out
}

fn routes(
    model: &Model,
    component: &Component,
    names: &SystemNames,
    edges: &[&MessageEdge],
) -> String {
    let mut out = text::inner_doc(&format!(
        "Message translations for {} (`{}`).\n\
         \n\
         This file belongs to the implementer. L2R created it once and will never\n\
         overwrite it. Each function must agree with the Lean translation it names.\n",
        component.name, component.id
    ));
    let mut modules: Vec<&str> = edges
        .iter()
        .flat_map(|e| [e.from.as_str(), e.to.as_str()])
        .filter_map(|id| model.component(id).map(|c| c.rust_module.as_str()))
        .collect();
    modules.sort();
    modules.dedup();
    out.push_str(&format!("\nuse super::{{{}}};\n", modules.join(", ")));
    for edge in edges {
        let from = &model.component(&edge.from).expect("validated").rust_module;
        let to = &model.component(&edge.to).expect("validated").rust_module;
        out.push_str(&format!(
            "\n/// Implements `{t}` (edge `{id}`).\n\
             pub(super) fn {function}(output: {from}::Output) -> Option<{to}::Input> {{\n    \
                 let _ = output;\n    \
                 todo!(\"implement {t}\")\n}}\n",
            t = edge.translation,
            id = edge.id,
            function = names.edge_function(&edge.id)
        ));
    }
    out
}
