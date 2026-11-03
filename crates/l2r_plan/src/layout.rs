//! Where each component's files live: its directory follows its containment path.

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
