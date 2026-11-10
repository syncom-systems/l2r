//! The output of successful validation: a model whose references are resolved.
//!
//! Planning accepts only a [`Validated`], so no generator code can run on a
//! model that has not passed every rule.

use std::collections::BTreeMap;

use l2r_model::{Inventory, Model};

use crate::data::DataType;
use crate::flat::Flat;

/// A canonical model and inventory that passed validation, with every Lean
/// reference resolved.
#[derive(Debug, Clone)]
pub struct Validated {
    model: Model,
    inventory: Inventory,
    machines: BTreeMap<String, ResolvedMachine>,
    port_messages: BTreeMap<String, ResolvedEnum>,
    types: BTreeMap<String, DataType>,
    flat: Option<Flat>,
}

/// A state machine component with its Lean types resolved to enumerations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMachine {
    /// Owned state type.
    pub state: ResolvedEnum,
    /// Input type.
    pub input: ResolvedEnum,
    /// Output type.
    pub output: ResolvedEnum,
}

/// A Lean inductive type whose constructors all have no fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEnum {
    /// Qualified Lean type name.
    pub lean_name: String,
    /// Constructors, in Lean declaration order.
    pub variants: Vec<Variant>,
}

/// One constructor and the Rust variant generated for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Qualified Lean constructor name.
    pub lean_name: String,
    /// Rust variant name.
    pub rust_name: String,
}

impl Validated {
    pub(crate) fn new(
        model: Model,
        inventory: Inventory,
        machines: BTreeMap<String, ResolvedMachine>,
        port_messages: BTreeMap<String, ResolvedEnum>,
    ) -> Self {
        Self {
            model,
            inventory,
            machines,
            port_messages,
            types: BTreeMap::new(),
            flat: None,
        }
    }

    pub(crate) fn new_v2(
        model: Model,
        inventory: Inventory,
        types: BTreeMap<String, DataType>,
        flat: Flat,
    ) -> Self {
        Self {
            model,
            inventory,
            machines: BTreeMap::new(),
            port_messages: BTreeMap::new(),
            types,
            flat: Some(flat),
        }
    }

    /// The flat network of a `v2` model: its machines, the routes their
    /// outputs take, and the root's ports. `None` for `v0` and `v1` models.
    pub fn flat(&self) -> Option<&Flat> {
        self.flat.as_ref()
    }

    /// Every Lean data type a `v2` model uses, by qualified name.
    pub fn types(&self) -> &BTreeMap<String, DataType> {
        &self.types
    }

    /// The data type of a qualified Lean name (`v2` models).
    pub fn data_type(&self, lean_name: &str) -> Option<&DataType> {
        self.types.get(lean_name)
    }

    /// The canonical model.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// The canonical inventory.
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// The resolved message type of a port id.
    pub fn port_message(&self, port_id: &str) -> Option<&ResolvedEnum> {
        self.port_messages.get(port_id)
    }

    /// The resolved machine for a component id, if it is a machine.
    pub fn machine(&self, component_id: &str) -> Option<&ResolvedMachine> {
        self.machines.get(component_id)
    }
}
