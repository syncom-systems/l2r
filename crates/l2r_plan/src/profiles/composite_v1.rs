//! Profile `composite/v1`: asynchronous composition with bounded mailboxes and
//! typed input ports.
//!
//! The semantics are `l2r.async/v1` (`lean/L2R/Async1.lean`). Compared with
//! `composite/v0`, which is unchanged:
//!
//! - every child has a fixed-capacity mailbox (an array and a length: no
//!   allocation, no loops, so the code can be translated for proof);
//! - a delivery is performed only if every receiver has room for what it
//!   produces; otherwise nothing changes and `step_once` reports `Blocked`
//!   (never `Settled`) when nothing else can run. Nothing is ever dropped;
//! - the environment reaches the composite only through typed ports
//!   (`send_<port>`), each returning an explicit `Rejected` when the target's
//!   mailbox is full.
//!
//! Generates `mod.rs` (generated) and `routes.rs` (implementation, holding one
//! translation per edge and per port).

use l2r_model::rust_names::composite_v1::SystemNames;
use l2r_model::{Component, MessageEdge, Port, profiles};
use l2r_validate::Validated;

use crate::plan::{Artifact, Ownership};
use crate::text;
use crate::{PlanError, planned_path};

/// Plans the files for one `composite/v1` in directory `dir`.
pub fn artifacts(
    validated: &Validated,
    component: &Component,
    dir: &str,
) -> Result<Vec<Artifact>, PlanError> {
    let model = validated.model();
    let children = model.children(&component.id);
    let edges = model.edges_within(&component.id);
    let ports: Vec<&Port> = model
        .ports
        .iter()
        .filter(|p| p.composite == component.id)
        .collect();
    let mut lean: Vec<String> = edges.iter().map(|e| e.translation.clone()).collect();
    lean.extend(
        ports
            .iter()
            .flat_map(|p| [p.translation_name().to_string(), p.message.clone()]),
    );
    lean.sort();
    lean.dedup();
    let mut sources = vec![component.id.clone()];
    sources.extend(children.iter().map(|c| c.id.clone()));
    let names = validated
        .composite_v1_names(&component.id)
        .ok_or_else(|| PlanError(format!("composite `{}` has no checked names", component.id)))?;
    let gen_ctx = Ctx {
        validated,
        names,
        component,
        children: &children,
        edges: &edges,
        ports: &ports,
    };
    Ok(vec![
        Artifact {
            path: planned_path(format!("{dir}/mod.rs"))?,
            ownership: Ownership::Generated,
            profile: profiles::COMPOSITE_V1.into(),
            purpose: "Composite contract: bounded mailboxes, typed ports and guarded scheduler"
                .into(),
            components: sources.clone(),
            lean: lean.clone(),
            content: gen_ctx.contract(),
        },
        Artifact {
            path: planned_path(format!("{dir}/routes.rs"))?,
            ownership: Ownership::Implementation,
            profile: profiles::COMPOSITE_V1.into(),
            purpose: "Message and port translations, owned by the implementer".into(),
            components: sources,
            lean,
            content: gen_ctx.routes(),
        },
    ])
}

struct Ctx<'a> {
    validated: &'a Validated,
    names: &'a SystemNames,
    component: &'a Component,
    children: &'a [&'a Component],
    edges: &'a [&'a MessageEdge],
    ports: &'a [&'a Port],
}

impl Ctx<'_> {
    fn capacity(&self, child: &Component) -> u32 {
        child.mailbox_capacity.unwrap_or(1)
    }

    fn module_of(&self, id: &str) -> &str {
        &self
            .validated
            .model()
            .component(id)
            .expect("validated reference")
            .rust_module
    }

    fn contract(&self) -> String {
        let mut out = text::generated_marker(profiles::COMPOSITE_V1);
        out.push('\n');
        out.push_str(&text::inner_doc(&self.module_doc()));
        out.push('\n');
        let mut modules: Vec<(String, &str)> = self
            .children
            .iter()
            .map(|c| (c.rust_module.clone(), "pub "))
            .collect();
        modules.push(("routes".into(), ""));
        modules.sort();
        for (module, visibility) in modules {
            out.push_str(&format!("{visibility}mod {module};\n"));
        }
        out.push('\n');
        for child in self.children {
            out.push_str(&format!(
                "/// Capacity of `{}`'s mailbox, from the architecture model.\npub const {}: usize = {};\n",
                child.id,
                self.names.capacity_const(&child.id),
                self.capacity(child)
            ));
        }
        out.push('\n');
        out.push_str(&self.port_messages());
        for child in self.children {
            out.push_str(&self.inbox(child));
        }
        out.push_str(&self.system());
        out.truncate(out.trim_end().len());
        out.push('\n');
        out
    }

    fn module_doc(&self) -> String {
        let c = self.component;
        let mut doc = format!(
            "{} (`{}`), profile `composite/v1`, semantics `l2r.async/v1`.\n\nResponsibility: {}\n",
            c.name, c.id, c.responsibility
        );
        if let Some(notes) = &c.notes {
            doc.push_str(&format!("\nArchitect's notes: {notes}\n"));
        }
        doc.push_str("\n# Composition\n\nChildren and mailbox capacities:\n\n");
        for child in self.children {
            doc.push_str(&format!(
                "- `{}`: {} (capacity {})\n",
                child.id,
                child.name,
                self.capacity(child)
            ));
        }
        doc.push_str("\nPorts (the only way the environment reaches the children):\n\n");
        for port in self.ports {
            doc.push_str(&format!(
                "- `{}` → `{}`: `{}` through `{}`\n",
                port.id,
                port.target(),
                port.message,
                port.translation_name()
            ));
        }
        doc.push_str("\nApproved message edges:\n\n");
        if self.edges.is_empty() {
            doc.push_str("- none\n");
        }
        for edge in self.edges {
            doc.push_str(&format!(
                "- `{}`: `{}` → `{}`, meaning `{}`\n",
                edge.id, edge.from, edge.to, edge.translation
            ));
        }
        doc.push_str(
            "\n# Behaviour\n\n\
             Each machine's state is owned by that machine and changed only by its own\n\
             `step`. Messages travel through bounded FIFO mailboxes. A delivery is\n\
             performed only if every receiver has room for what it produces; otherwise\n\
             nothing changes and the sender is *blocked*. `System::step_once` reports\n\
             `Progress::Delivered`, `Progress::Settled` (nothing pending) or\n\
             `Progress::Blocked` (messages pending, no delivery enabled). A port send\n\
             returns `Err(Rejected)` when the target's mailbox is full; the caller keeps\n\
             the message. No message is dropped for lack of room. A translation that\n\
             returns `None` is the Lean specification's own decision that an output\n\
             means nothing to the receiver.\n\n\
             The scheduler serves children round-robin in identifier order. This is one\n\
             deterministic scheduler; the Lean semantics permit every interleaving.\n\
             When events may arrive is an assumption about the environment, stated and\n\
             verified separately from this interface.\n",
        );
        doc
    }

    fn port_messages(&self) -> String {
        let mut out = String::new();
        let mut done: Vec<String> = Vec::new();
        for port in self.ports {
            let ty = self.names.message_type(&port.id).to_string();
            if done.contains(&ty) {
                continue;
            }
            done.push(ty.clone());
            let Some(resolved) = self.validated.port_message(&port.id) else {
                continue;
            };
            out.push_str(&format!(
                "/// Messages accepted by ports of Lean type `{}`.\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {ty} {{\n",
                port.message
            ));
            for variant in &resolved.variants {
                out.push_str(&format!(
                    "    /// `{}`\n    {},\n",
                    variant.lean_name, variant.rust_name
                ));
            }
            out.push_str("}\n\n");
        }
        out
    }

    fn inbox(&self, child: &Component) -> String {
        let m = &child.rust_module;
        let ty = self.names.inbox_type(&child.id);
        let n = self.capacity(child) as usize;
        let cap = self.names.capacity_const(&child.id);
        let nones = vec!["None"; n].join(", ");
        let mut shift = String::new();
        for i in 0..n.saturating_sub(1) {
            shift.push_str(&format!(
                "        self.slots[{i}] = self.slots[{}];\n",
                i + 1
            ));
        }
        format!(
            "/// The bounded FIFO mailbox of `{id}`: at most `{cap}` messages, oldest first.\n\
             #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
             pub struct {ty} {{\n    slots: [Option<{m}::Input>; {n}],\n    len: usize,\n}}\n\n\
             impl {ty} {{\n\
             \x20   fn new() -> Self {{\n        Self {{\n            slots: [{nones}],\n            len: 0,\n        }}\n    }}\n\n\
             \x20   /// Number of messages held.\n    pub fn len(&self) -> usize {{\n        self.len\n    }}\n\n\
             \x20   /// True when the mailbox holds no message.\n    pub fn is_empty(&self) -> bool {{\n        self.len == 0\n    }}\n\n\
             \x20   /// True when the mailbox is at capacity.\n    pub fn is_full(&self) -> bool {{\n        self.len >= {cap}\n    }}\n\n\
             \x20   /// The message at `index`, counting from the oldest.\n    pub fn get(&self, index: usize) -> Option<{m}::Input> {{\n        if index >= self.len {{\n            return None;\n        }}\n        self.slots[index]\n    }}\n\n\
             \x20   fn peek(&self) -> Option<{m}::Input> {{\n        self.slots[0]\n    }}\n\n\
             \x20   /// Appends `input`; returns `false`, changing nothing, when the mailbox is full.\n    fn push(&mut self, input: {m}::Input) -> bool {{\n        if self.len >= {cap} {{\n            return false;\n        }}\n        self.slots[self.len] = Some(input);\n        self.len += 1;\n        true\n    }}\n\n\
             \x20   /// Removes and returns the oldest message.\n    fn pop(&mut self) -> Option<{m}::Input> {{\n        if self.len == 0 {{\n            return None;\n        }}\n        let first = self.slots[0];\n{shift}        self.slots[{last}] = None;\n        self.len -= 1;\n        first\n    }}\n}}\n\n",
            id = child.id,
            last = n - 1,
        )
    }

    fn system(&self) -> String {
        let n = self.children.len();
        let mut out = String::from(
            "/// A port refused a message because its target's mailbox is full. The caller\n\
             /// keeps the message; nothing was changed or lost.\n\
             #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct Rejected;\n\n\
             /// What one call of `System::step_once` did.\n\
             #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Progress {\n\
             \x20   /// One message was delivered.\n    Delivered,\n\
             \x20   /// Nothing is pending.\n    Settled,\n\
             \x20   /// Messages are pending, but every delivery is blocked by a full receiver.\n    Blocked,\n}\n\n\
             /// `System::run` stopped before the system settled.\n\
             #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Stopped {\n\
             \x20   /// The step budget ran out with deliveries still enabled.\n    Budget {\n        /// Deliveries performed.\n        steps: usize,\n    },\n\
             \x20   /// Messages are pending but every delivery is blocked.\n    Blocked {\n        /// Deliveries performed before blocking.\n        steps: usize,\n    },\n}\n\n\
             enum Attempt {\n    Delivered,\n    Blocked,\n    Idle,\n}\n\n\
             /// The composed system: one owned state and one bounded mailbox per child.\n\
             #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct System {\n",
        );
        out.push_str(&self.system_fields());
        out.push_str("    next: usize,\n}\n\n");
        out.push_str(&format!(
            "impl System {{\n    const CHILDREN: usize = {n};\n\n"
        ));
        out.push_str(&self.constructor());
        out.push_str(&self.accessors());
        out.push_str(&self.sends());
        out.push_str(
            "    /// True when no mailbox holds a message.\n    pub fn is_settled(&self) -> bool {\n        let mut settled = true;\n",
        );
        for child in self.children {
            out.push_str(&format!(
                "        settled &= self.{}.is_empty();\n",
                self.names.inbox(&child.id)
            ));
        }
        out.push_str("        settled\n    }\n\n");
        // step_once, unrolled
        out.push_str(
            "    /// Serves the next child, in round-robin order, whose delivery is enabled.\n    pub fn step_once(&mut self) -> Progress {\n        let mut blocked = false;\n",
        );
        for offset in 0..n {
            let position = if offset == 0 {
                "self.next".to_string()
            } else {
                format!("self.next + {offset}")
            };
            out.push_str(&format!(
                "        let child = ({position}) % Self::CHILDREN;\n        match self.attempt(child) {{\n            Attempt::Delivered => {{\n                self.next = (child + 1) % Self::CHILDREN;\n                return Progress::Delivered;\n            }}\n            Attempt::Blocked => blocked = true,\n            Attempt::Idle => {{}}\n        }}\n"
            ));
        }
        out.push_str(
            "        if blocked {\n            Progress::Blocked\n        } else {\n            Progress::Settled\n        }\n    }\n\n",
        );
        out.push_str(
            "    /// Delivers until settled, blocked or `max_steps` deliveries.\n    pub fn run(&mut self, max_steps: usize) -> Result<usize, Stopped> {\n        for steps in 0..max_steps {\n            match self.step_once() {\n                Progress::Delivered => {}\n                Progress::Settled => return Ok(steps),\n                Progress::Blocked => return Err(Stopped::Blocked { steps }),\n            }\n        }\n        if self.is_settled() {\n            Ok(max_steps)\n        } else {\n            Err(Stopped::Budget { steps: max_steps })\n        }\n    }\n\n",
        );
        // attempt dispatch
        out.push_str(
            "    fn attempt(&mut self, child: usize) -> Attempt {\n        match child {\n",
        );
        for (index, child) in self.children.iter().enumerate() {
            out.push_str(&format!(
                "            {index} => self.{}(),\n",
                self.names.attempt(&child.id)
            ));
        }
        out.push_str("            _ => Attempt::Idle,\n        }\n    }\n\n");
        for child in self.children {
            out.push_str(&self.attempt_fn(child));
        }
        out.truncate(out.trim_end().len());
        out.push_str("\n}\n\nimpl Default for System {\n    fn default() -> Self {\n        Self::new()\n    }\n}\n");
        out
    }

    /// The fields of `System` for the children.
    fn system_fields(&self) -> String {
        let mut out = String::new();
        for child in self.children {
            let m = &child.rust_module;
            out.push_str(&format!(
                "    {}: {m}::State,\n    {}: {},\n",
                self.names.field(&child.id),
                self.names.inbox(&child.id),
                self.names.inbox_type(&child.id)
            ));
        }
        out
    }

    /// `System::new`.
    fn constructor(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "    /// Every child in its initial state, with empty mailboxes.\n    pub fn new() -> Self {\n        Self {\n",
        );
        for child in self.children {
            let m = &child.rust_module;
            out.push_str(&format!(
                "            {}: {m}::init(),\n            {}: {}::new(),\n",
                self.names.field(&child.id),
                self.names.inbox(&child.id),
                self.names.inbox_type(&child.id)
            ));
        }
        out.push_str("            next: 0,\n        }\n    }\n\n");
        out
    }

    /// The read accessors of each child's state and mailbox.
    fn accessors(&self) -> String {
        let mut out = String::new();
        for child in self.children {
            let m = &child.rust_module;
            out.push_str(&format!(
                "    /// Current state of `{id}`. Read-only: only `{m}::step` changes it.\n    pub fn {field}(&self) -> {m}::State {{\n        self.{field}\n    }}\n\n\
                 \x20   /// The mailbox of `{id}`, read-only.\n    pub fn {inbox}(&self) -> &{ib} {{\n        &self.{inbox}\n    }}\n\n",
                id = child.id,
                field = self.names.field(&child.id),
                inbox = self.names.inbox(&child.id),
                ib = self.names.inbox_type(&child.id)
            ));
        }
        out
    }

    /// One `send_<port>` per port.
    fn sends(&self) -> String {
        let mut out = String::new();
        for port in self.ports {
            out.push_str(&format!(
                "    /// Port `{id}`: offers `message` to `{to}`. Accepted only if its mailbox has room;\n    /// otherwise `Err(Rejected)`, and the caller keeps the message.\n    pub fn {send}(&mut self, message: {ty}) -> Result<(), Rejected> {{\n        let input = routes::{function}(message);\n        if self.{target_inbox}.push(input) {{\n            Ok(())\n        }} else {{\n            Err(Rejected)\n        }}\n    }}\n\n",
                id = port.id,
                to = port.target(),
                send = self.names.send(&port.id),
                function = self.names.port_function(&port.id),
                target_inbox = self.names.inbox(port.target()),
                ty = self.names.message_type(&port.id),
            ));
        }
        out
    }

    fn attempt_fn(&self, child: &Component) -> String {
        let m = &child.rust_module;
        let attempt = self.names.attempt(&child.id);
        let field = self.names.field(&child.id);
        let inbox = self.names.inbox(&child.id);
        let outgoing: Vec<&&MessageEdge> =
            self.edges.iter().filter(|e| e.from == child.id).collect();
        let mut out = format!(
            "    fn {attempt}(&mut self) -> Attempt {{\n        let input = match self.{inbox}.peek() {{\n            Some(input) => input,\n            None => return Attempt::Idle,\n        }};\n        let (state, output) = {m}::step(self.{field}, input);\n"
        );
        for edge in &outgoing {
            out.push_str(&format!(
                "        let to_{id} = match output {{\n            Some(output) => routes::{function}(output),\n            None => None,\n        }};\n",
                id = edge.id,
                function = self.names.edge_function(&edge.id)
            ));
        }
        for edge in &outgoing {
            out.push_str(&format!(
                "        if to_{id}.is_some() && self.{target_inbox}.is_full() {{\n            return Attempt::Blocked;\n        }}\n",
                id = edge.id,
                target_inbox = self.names.inbox(&edge.to)
            ));
        }
        out.push_str(&format!(
            "        let _ = self.{inbox}.pop();\n        self.{field} = state;\n"
        ));
        for edge in &outgoing {
            out.push_str(&format!(
                "        if let Some(message) = to_{id} {{\n            let _ = self.{target_inbox}.push(message);\n        }}\n",
                id = edge.id,
                target_inbox = self.names.inbox(&edge.to)
            ));
        }
        out.push_str("        Attempt::Delivered\n    }\n\n");
        out
    }

    fn routes(&self) -> String {
        let c = self.component;
        let mut out = text::inner_doc(&format!(
            "Message and port translations for {} (`{}`).\n\
             \n\
             This file belongs to the implementer. L2R created it once and will never\n\
             overwrite it. Each function must agree with the Lean translation it names.\n\
             Port translations are total: a port message always becomes an input.\n",
            c.name, c.id
        ));
        let mut modules: Vec<String> = self
            .edges
            .iter()
            .flat_map(|e| {
                [
                    self.module_of(&e.from).to_string(),
                    self.module_of(&e.to).to_string(),
                ]
            })
            .chain(
                self.ports
                    .iter()
                    .map(|p| self.module_of(p.target()).to_string()),
            )
            .collect();
        modules.sort();
        modules.dedup();
        let mut imports = modules;
        let mut types: Vec<String> = self
            .ports
            .iter()
            .map(|p| self.names.message_type(&p.id).to_string())
            .collect();
        types.sort();
        types.dedup();
        imports.extend(types);
        imports.sort();
        out.push_str(&format!("\nuse super::{{{}}};\n", imports.join(", ")));
        for edge in self.edges {
            let from = self.module_of(&edge.from);
            let to = self.module_of(&edge.to);
            out.push_str(&format!(
                "\n/// Implements `{t}` (edge `{id}`). `None` means the output means nothing to `{to_id}`.\n\
                 pub(super) fn {function}(output: {from}::Output) -> Option<{to}::Input> {{\n    \
                     let _ = output;\n    \
                     todo!(\"implement {t}\")\n}}\n",
                t = edge.translation,
                id = edge.id,
                function = self.names.edge_function(&edge.id),
                to_id = edge.to
            ));
        }
        for port in self.ports {
            let to = self.module_of(port.target());
            out.push_str(&format!(
                "\n/// Implements `{t}` (port `{id}`).\n\
                 pub(super) fn {function}(message: {ty}) -> {to}::Input {{\n    \
                     let _ = message;\n    \
                     todo!(\"implement {t}\")\n}}\n",
                t = port.translation_name(),
                id = port.id,
                function = self.names.port_function(&port.id),
                ty = self.names.message_type(&port.id),
            ));
        }
        out
    }
}
