//! Where each component's files live, for `v2` models: a component's directory
//! follows its containment path.
//!
//! Frozen with the `v2` profiles (audit Q4). It starts as a copy of the `v0`/`v1`
//! layout; the two are kept apart so that a later version can change one
//! without moving a file of the other.

use std::collections::BTreeMap;

use l2r_model::Model;

/// Maps each component id to its source directory, e.g. `src/crossing/signal`.
pub fn component_dirs(model: &Model) -> BTreeMap<String, String> {
    let mut dirs = BTreeMap::new();
    for component in &model.components {
        let mut modules = vec![component.rust_module.as_str()];
        let mut parent = component.parent.as_deref();
        while let Some(id) = parent {
            let Some(p) = model.component(id) else { break };
            modules.push(p.rust_module.as_str());
            parent = p.parent.as_deref();
        }
        modules.reverse();
        dirs.insert(component.id.clone(), format!("src/{}", modules.join("/")));
    }
    dirs
}
