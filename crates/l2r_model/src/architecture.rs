//! The architectural model: the authoritative source for software *structure*.
//!
//! The model owns component identities, containment, responsibilities, state
//! ownership, permitted message edges and Rust mappings. It deliberately does not
//! restate behavior: states, inputs, transitions and invariants are referenced by
//! qualified Lean name and resolved against a Lean declaration inventory.

use serde::{Deserialize, Serialize};

/// The original model schema. Models in it cannot use ports, capacities or
/// the `composite/v1` profile.
pub const MODEL_SCHEMA: &str = "l2r.architecture/v0";

/// The model schema that adds typed composite ports and mailbox capacities
/// (profile `composite/v1`). Every `v0` model is also accepted unchanged.
pub const MODEL_SCHEMA_V1: &str = "l2r.architecture/v1";

/// The model schema for hierarchical composition (profile `composite/v2`):
/// nested composites, input and output ports, and machines whose Lean types
/// carry data (profile `transition-core/v1`). A `v2` model uses only those two
/// component profiles; `v0` and `v1` models are unchanged.
pub const MODEL_SCHEMA_V2: &str = "l2r.architecture/v2";

/// Every model schema this generator accepts.
pub const SUPPORTED_SCHEMAS: &[&str] = &[MODEL_SCHEMA, MODEL_SCHEMA_V1, MODEL_SCHEMA_V2];

/// A complete architectural model, as authored by the architect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Schema identifier; one of [`SUPPORTED_SCHEMAS`].
    pub schema: String,
    /// System-wide settings.
    pub system: System,
    /// Every component, in any order; containment is given by `parent`.
    #[serde(default, rename = "component")]
    pub components: Vec<Component>,
    /// Permitted asynchronous message edges between components.
    #[serde(default, rename = "message")]
    pub messages: Vec<MessageEdge>,
    /// Typed input ports of composites (schema `v1`).
    #[serde(default, rename = "port", skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<Port>,
}

/// System-wide identity and locations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct System {
    /// Stable system identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Name of the generated Rust crate (and its Bazel target).
    pub rust_crate: String,
    /// Lean declaration inventory, relative to the model file.
    pub lean_inventory: String,
    /// Directory receiving generated files, relative to the model file.
    pub output_dir: String,
}

/// One architectural component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// Stable identifier, unique within the model.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Containing component; `None` only for the single root.
    #[serde(default)]
    pub parent: Option<String>,
    /// Versioned generation profile, which also fixes the component's kind.
    pub profile: String,
    /// What this component is responsible for, in the architect's words.
    pub responsibility: String,
    /// Rust module name for this component, unique among its siblings.
    pub rust_module: String,
    /// Instructions from the architect to the implementer.
    #[serde(default)]
    pub notes: Option<String>,
    /// Upper bound on the size of implementation files, in lines.
    #[serde(default)]
    pub implementation_budget_lines: Option<u32>,
    /// Lean references for a state machine; required by machine profiles.
    #[serde(default)]
    pub behavior: Option<Behavior>,
    /// Capacity of this machine's mailbox, in messages (schema `v1`; required
    /// for the children of a `composite/v1`). Part of the behavioral
    /// semantics: a full mailbox blocks senders; nothing is dropped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mailbox_capacity: Option<u32>,
}

/// Qualified Lean references describing one state machine.
///
/// The component owning a `Behavior` is the single owner of `owned_state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Behavior {
    /// Lean type of the state this component exclusively owns.
    pub owned_state: String,
    /// Lean type of the inputs the machine accepts.
    pub input: String,
    /// Lean type of the outputs the machine emits.
    pub output: String,
    /// Lean definition of the initial state.
    pub init: String,
    /// Lean transition function `State → Input → State × Option Output`.
    pub step: String,
    /// Lean theorems the implementation is obliged to respect.
    #[serde(default)]
    pub invariants: Vec<String>,
}

/// A permitted message edge: outputs of `from` may be delivered to `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageEdge {
    /// Stable identifier; also the name of the generated routing function.
    pub id: String,
    /// Sending component.
    pub from: String,
    /// Receiving component.
    pub to: String,
    /// Lean function `FromOutput → Option ToInput` giving the message's meaning.
    pub translation: String,
}

/// A typed port of a composite: the only way messages cross its boundary.
///
/// Schema `v1` has input ports only, each feeding one child machine. Schema
/// `v2` adds a `direction`: an `in` port forwards to one child machine or to an
/// `in` port of a child composite; an `out` port is fed by message edges inside
/// its composite and, on the root, holds what the system emits to the
/// environment in a bounded outbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    /// Stable identifier; also names the generated `send_<id>` or `take_<id>`.
    pub id: String,
    /// The composite this port belongs to.
    pub composite: String,
    /// What an input port feeds: a child machine, or (schema `v2`) an `in`
    /// port of a child composite. Absent on `out` ports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Lean type of the messages the port carries.
    pub message: String,
    /// Lean function `PortMessage → Target` (total) giving an input port
    /// message's meaning to what the port feeds. Absent on `out` ports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    /// `"in"` or `"out"` (schema `v2`; `v1` ports are all inputs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Capacity of the outbox of an `out` port of the root composite (schema
    /// `v2`). Part of the behavioral semantics: a full outbox blocks senders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<u32>,
}

impl Port {
    /// The port's target, or the empty string when it has none.
    pub fn target(&self) -> &str {
        self.to.as_deref().unwrap_or_default()
    }

    /// The port's translation, or the empty string when it has none.
    pub fn translation_name(&self) -> &str {
        self.translation.as_deref().unwrap_or_default()
    }

    /// True for an `out` port (schema `v2`).
    pub fn is_out(&self) -> bool {
        self.direction.as_deref() == Some("out")
    }
}

impl Model {
    /// Returns the model in canonical form: order-insensitive collections sorted.
    ///
    /// Two models that differ only in the order of components, messages or
    /// invariants have the same canonical form, and therefore the same digest.
    pub fn canonical(&self) -> Model {
        let mut model = self.clone();
        model.components.sort_by(|a, b| a.id.cmp(&b.id));
        model.messages.sort_by(|a, b| a.id.cmp(&b.id));
        model.ports.sort_by(|a, b| a.id.cmp(&b.id));
        for component in &mut model.components {
            if let Some(behavior) = &mut component.behavior {
                behavior.invariants.sort();
            }
        }
        model
    }

    /// Finds a component by identifier.
    pub fn component(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }

    /// Returns the components whose parent is `id`, in canonical order.
    pub fn children(&self, id: &str) -> Vec<&Component> {
        let mut children: Vec<&Component> = self
            .components
            .iter()
            .filter(|c| c.parent.as_deref() == Some(id))
            .collect();
        children.sort_by(|a, b| a.id.cmp(&b.id));
        children
    }
}
