//! Planning for model schema `v2`: hierarchical composition with data-carrying
//! types.
//!
//! Three profiles, all new versions beside the unchanged `v0`/`v1` ones:
//!
//! - `crate-root/v1`: the Bazel package, the crate root, and `types.rs`, which
//!   holds exactly one Rust type for every Lean data type the model uses;
//! - `transition-core/v1`: a machine whose types are aliases into `types.rs`;
//! - `composite/v2`: a module for each composite, with the translations of the
//!   edges and input ports it owns; the root composite additionally holds the
//!   `System` that schedules the flat network of every machine.

mod composite;
mod crate_root;
mod machine;
mod system;
mod types;

use std::collections::{BTreeMap, BTreeSet};

use l2r_model::{Component, Model, profiles as profile_ids};
use l2r_validate::Validated;
use l2r_validate::flat::{Flat, Hop, HopKind};

use crate::plan::Artifact;
use crate::{PlanError, layout};

/// Everything the `v2` profiles need to know about one validated model.
pub(crate) struct Gen<'a> {
    pub validated: &'a Validated,
    pub model: &'a Model,
    pub flat: &'a Flat,
    pub root: &'a Component,
    pub dirs: BTreeMap<String, String>,
}

impl<'a> Gen<'a> {
    pub fn new(validated: &'a Validated) -> Result<Self, PlanError> {
        let model = validated.model();
        let flat = validated
            .flat()
            .ok_or_else(|| PlanError("a v2 model without a flat network".into()))?;
        let root = model
            .components
            .iter()
            .find(|c| c.parent.is_none())
            .ok_or_else(|| PlanError("no root component".into()))?;
        Ok(Self {
            validated,
            model,
            flat,
            root,
            dirs: layout::component_dirs(model),
        })
    }

    /// The Rust type of a Lean data type, from the crate root.
    pub fn type_path(&self, lean_name: &str) -> Result<String, PlanError> {
        self.validated
            .data_type(lean_name)
            .map(|t| t.rust_path())
            .ok_or_else(|| PlanError(format!("type `{lean_name}` was not resolved")))
    }

    /// The modules from the root composite down to `id`, excluding the root:
    /// the path of a component as seen from the root composite's module.
    pub fn modules_below_root(&self, id: &str) -> Vec<&'a str> {
        let mut modules = Vec::new();
        let mut current = self.model.component(id);
        while let Some(component) = current {
            if component.parent.is_none() {
                break;
            }
            modules.push(component.rust_module.as_str());
            current = component
                .parent
                .as_deref()
                .and_then(|p| self.model.component(p));
        }
        modules.reverse();
        modules
    }

    /// `sensing::filter` for a machine, as seen from the root composite's module.
    pub fn machine_module(&self, id: &str) -> String {
        self.modules_below_root(id).join("::")
    }

    /// The function implementing a hop, as seen from the root composite's module.
    pub fn hop_function(&self, hop: &Hop) -> String {
        let mut path = self.modules_below_root(&hop.owner);
        path.push("routes");
        let name = hop_function_name(hop);
        format!("{}::{name}", path.join("::"))
    }

    /// The hops implemented in `composite`'s `routes.rs`, by function name.
    pub fn hops_owned_by(&self, composite: &str) -> Vec<&'a Hop> {
        let mut seen = BTreeSet::new();
        let mut owned: Vec<&Hop> = self
            .flat
            .leaves
            .iter()
            .flat_map(|l| l.routes.iter().flat_map(|r| r.hops.iter()))
            .chain(self.flat.inputs.iter().flat_map(|i| i.hops.iter()))
            .filter(|h| h.owner == composite && seen.insert(hop_function_name(h)))
            .collect();
        owned.sort_by_key(|h| (h.kind == HopKind::Port, h.id.clone()));
        owned
    }
}

/// `filter_to_level` for an edge, `port_reading` for an input port.
pub(crate) fn hop_function_name(hop: &Hop) -> String {
    match hop.kind {
        HopKind::Edge => hop.id.clone(),
        HopKind::Port => format!("port_{}", hop.id),
    }
}

/// Plans every file of a `v2` model and names the profiles used.
pub fn artifacts(validated: &Validated) -> Result<(Vec<Artifact>, BTreeSet<String>), PlanError> {
    let generator = Gen::new(validated)?;
    let mut artifacts = crate_root::artifacts(&generator)?;
    artifacts.push(types::artifact(&generator)?);
    let mut profiles = BTreeSet::from([crate_root::PROFILE.to_string()]);
    for component in &generator.model.components {
        profiles.insert(component.profile.clone());
        match component.profile.as_str() {
            profile_ids::TRANSITION_CORE_V1 => {
                artifacts.extend(machine::artifacts(&generator, component)?)
            }
            profile_ids::COMPOSITE_V2 => {
                artifacts.extend(composite::artifacts(&generator, component)?)
            }
            other => {
                return Err(PlanError(format!(
                    "profile `{other}` has no v2 implementation"
                )));
            }
        }
    }
    Ok((artifacts, profiles))
}
