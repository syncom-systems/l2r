//! `l2r packet`: the work packet for implementing one component.

use std::path::Path;

use l2r_model::{Behavior, Component, MessageEdge, Model, Port, Schema};
use l2r_plan::{Artifact, Ownership};
use l2r_validate::flat::Flat;

use crate::commands::Outcome;
use crate::session::{Session, SessionError};

/// `l2r packet <model> <component>`: everything an implementer (human or
/// agent) needs to work on one component, and nothing that invites changing
/// the architecture.
pub fn packet(model: &Path, component_id: &str) -> Outcome {
    let session = match Session::open(model) {
        Ok(session) => session,
        Err(SessionError::Load(message) | SessionError::Plan(message)) => {
            return Outcome::refused(format!("{message}\n"));
        }
        Err(SessionError::Invalid(diagnostics)) => {
            let mut text: String = diagnostics.iter().map(|d| format!("{d}\n")).collect();
            text.push_str("model rejected: a work packet is issued only for a valid model\n");
            return Outcome::refused(text);
        }
    };
    let model_data = session.model();
    let Some(component) = model_data.component(component_id) else {
        let ids: Vec<&str> = model_data
            .components
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        return Outcome::refused(format!(
            "no component `{component_id}`; components: {}\n",
            ids.join(", ")
        ));
    };
    let packet = WorkPacket {
        session: &session,
        component,
    };
    Outcome::ok(packet.text())
}

/// What the implementer is held to, whatever the component.
const RULES: &str = "\n## Rules\n\n\
     - Edit only the files listed under \"Files you may edit\". Generated files are\n  \
       rewritten from the model; `l2r check` reports any edit to them as drift.\n\
     - Do not add state, I/O, clocks, public items or dependencies. The component's\n  \
       state is exactly its Lean state type; only its own transition changes it.\n\
     - Implement what the Lean declarations say, including integer arithmetic\n  \
       (Lean's fixed-width integers wrap; Rust's `+` does not).\n\
     - If the contract cannot be satisfied, or the specification looks wrong, stop\n  \
       and report it. Do not change the model, the Lean specification, the tests or\n  \
       the approval record to make something pass: those belong to the architect.\n\
     - Passing Rust tests is not evidence of agreement with Lean; say which evidence\n  \
       you have (tests, conformance run, refinement proof) and which you do not.\n";

/// A function of `routes.rs` and the Lean translation it implements.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Translation {
    function: String,
    lean: String,
    lean_type: String,
}

/// The work packet of one component of a planned model, as Markdown.
struct WorkPacket<'a> {
    session: &'a Session,
    component: &'a Component,
}

impl WorkPacket<'_> {
    fn text(&self) -> String {
        [
            self.heading(),
            self.editable_files(),
            self.contract(),
            self.behaviour(),
            self.theorems(),
            self.translations(),
            self.connections(),
            RULES.to_string(),
        ]
        .concat()
    }

    fn model(&self) -> &Model {
        self.session.model()
    }

    /// The Lean type of a declaration, as the inventory records it.
    fn type_of(&self, name: &str) -> String {
        self.session
            .validated
            .inventory()
            .find(name)
            .map(|d| d.type_.clone())
            .unwrap_or_else(|| "(not in the inventory)".into())
    }

    /// The planned files this component owns, of one ownership class.
    fn owned(&self, ownership: Ownership) -> impl Iterator<Item = &Artifact> {
        let id = &self.component.id;
        self.session
            .plan
            .artifacts
            .iter()
            .filter(move |a| a.components.first() == Some(id) && a.ownership == ownership)
    }

    fn file_line(&self, artifact: &Artifact) -> String {
        format!(
            "- `{}/{}`: {}\n",
            self.session.plan.output_dir, artifact.path, artifact.purpose
        )
    }

    /// Title, identity of the plan, responsibility and the architect's notes.
    fn heading(&self) -> String {
        let component = self.component;
        let mut out = format!(
            "# Work packet: {} (`{}`)\n\n\
             System `{}`, profile `{}`{}. Plan digest `{}`.\n\n\
             ## Responsibility\n\n{}\n",
            component.name,
            component.id,
            self.model().system.id,
            component.profile,
            component
                .parent
                .as_ref()
                .map(|p| format!(", inside `{p}`"))
                .unwrap_or_default(),
            self.session.plan.digest(),
            component.responsibility
        );
        if let Some(notes) = &component.notes {
            out.push_str(&format!("\n## Architect's notes\n\n{notes}\n"));
        }
        out
    }

    fn editable_files(&self) -> String {
        let mut out = String::from("\n## Files you may edit\n\n");
        let editable: Vec<String> = self
            .owned(Ownership::Implementation)
            .map(|a| self.file_line(a))
            .collect();
        if editable.is_empty() {
            out.push_str("None. This component has no implementation file.\n");
        }
        out.push_str(&editable.concat());
        if let Some(budget) = self.component.implementation_budget_lines {
            out.push_str(&format!(
                "\nSize budget: at most {budget} lines per implementation file (`l2r check` enforces it).\n"
            ));
        }
        out
    }

    fn contract(&self) -> String {
        let plan = &self.session.plan;
        let mut out = String::from("\n## Contract: read, never edit\n\n");
        out.extend(self.owned(Ownership::Generated).map(|a| self.file_line(a)));
        if plan.artifact("src/types.rs").is_some() {
            out.push_str(&format!(
                "- `{}/src/types.rs`: the Rust mirror of every Lean data type\n",
                plan.output_dir
            ));
        }
        out
    }

    /// The Lean declarations a machine implements. Empty for a composite.
    fn behaviour(&self) -> String {
        let Some(behavior) = &self.component.behavior else {
            return String::new();
        };
        let mut out = format!(
            "\n## Behaviour to implement (Lean is authoritative)\n\n\
             | Role | Lean declaration | Type |\n| --- | --- | --- |\n\
             | owned state | `{}` | |\n| input | `{}` | |\n| output | `{}` | |\n\
             | initial state | `{}` | `{}` |\n| transition | `{}` | `{}` |\n",
            behavior.owned_state,
            behavior.input,
            behavior.output,
            behavior.init,
            self.type_of(&behavior.init),
            behavior.step,
            self.type_of(&behavior.step),
        );
        if let Some(capacity) = self.component.mailbox_capacity {
            out.push_str(&format!("\nMailbox capacity: {capacity} messages.\n"));
        }
        out
    }

    /// The theorems a machine's behaviour refers to. Empty for a composite.
    fn theorems(&self) -> String {
        let Some(Behavior { invariants, .. }) = &self.component.behavior else {
            return String::new();
        };
        let mut out = String::from(
            "\n## Theorems about the specification\n\n\
             Proved in Lean about the specification. The implementation inherits them only\n\
             if it computes exactly what the transition says.\n\n",
        );
        if invariants.is_empty() {
            out.push_str("(None referenced.)\n");
        }
        for invariant in invariants {
            out.push_str(&format!("- `{invariant}`: `{}`\n", self.type_of(invariant)));
        }
        out
    }

    fn translation(&self, function: String, lean: &str) -> Translation {
        Translation {
            function,
            lean: lean.to_string(),
            lean_type: self.type_of(lean),
        }
    }

    /// The translations on the flattened routes that this component's
    /// `routes.rs` implements, each function once.
    fn routed_translations(&self, flat: &Flat) -> Vec<Translation> {
        flat.hops_owned_by(&self.component.id)
            .into_iter()
            .map(|hop| self.translation(hop.function(), &hop.translation))
            .collect()
    }

    /// The translations of a model that has no flattened routes: those of the
    /// edges its children send on, then those of its own ports.
    fn declared_translations(&self) -> Vec<Translation> {
        let model = self.model();
        // A routes.rs function of an edge is named by the edge's own id.
        let edges = model
            .edges_within(&self.component.id)
            .into_iter()
            .map(|edge| self.translation(edge.id.clone(), &edge.translation));
        // Only a composite/v1 has ports, and its checked names say what their
        // functions are called.
        let v1_names = self
            .session
            .validated
            .composite_v1_names(&self.component.id);
        let ports = v1_names.into_iter().flat_map(|names| {
            model
                .ports
                .iter()
                .filter(|port| port.composite == self.component.id)
                .map(|port| {
                    let function = names.port_function(&port.id).to_string();
                    self.translation(function, port.translation_name())
                })
        });
        edges.chain(ports).collect()
    }

    /// The translations this component's files implement. Empty when there
    /// are none.
    fn translations(&self) -> String {
        let validated = &self.session.validated;
        let mut translations = match (validated.schema(), validated.flat()) {
            (Schema::V2, Some(flat)) => self.routed_translations(flat),
            (Schema::V2, None) | (Schema::V0 | Schema::V1, _) => self.declared_translations(),
        };
        if translations.is_empty() {
            return String::new();
        }
        translations.sort();
        let mut out = String::from(
            "\n## Translations to implement (in `routes.rs`)\n\n\
             | Function | Lean declaration | Type |\n| --- | --- | --- |\n",
        );
        for translation in &translations {
            let Translation {
                function,
                lean,
                lean_type,
            } = translation;
            out.push_str(&format!("| `{function}` | `{lean}` | `{lean_type}` |\n"));
        }
        out
    }

    /// Where the component sits in the message structure: the edges it sends
    /// or receives on, then the ports it owns or is fed by. Empty when there
    /// are none.
    fn connections(&self) -> String {
        let id = &self.component.id;
        let model = self.model();
        let edges = model
            .messages
            .iter()
            .filter(|edge| edge.from == *id || edge.to == *id)
            .map(edge_line);
        let ports = model
            .ports
            .iter()
            .filter(|port| port.composite == *id || port.target() == id)
            .map(port_line);
        let connections: String = edges.chain(ports).collect();
        if connections.is_empty() {
            return String::new();
        }
        format!("\n## Approved connections\n\n{connections}")
    }
}

fn edge_line(edge: &MessageEdge) -> String {
    format!(
        "- edge `{}`: `{}` → `{}` through `{}`\n",
        edge.id, edge.from, edge.to, edge.translation
    )
}

fn port_line(port: &Port) -> String {
    let direction = if port.is_out() { "out" } else { "in" };
    let target = match port.target() {
        "" => String::new(),
        target => format!(" → `{target}` through `{}`", port.translation_name()),
    };
    format!(
        "- {direction} port `{}` of `{}`: `{}`{target}\n",
        port.id, port.composite, port.message
    )
}
