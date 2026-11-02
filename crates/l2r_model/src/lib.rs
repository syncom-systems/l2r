//! Typed, versioned inputs to L2R: the architectural model and the Lean
//! declaration inventory it references.

pub mod architecture;
pub mod digest;
pub mod lean;
pub mod load;
pub mod profiles;

pub use architecture::{Behavior, Component, MODEL_SCHEMA, MessageEdge, Model, System};
pub use lean::{Constructor, DeclKind, Declaration, INVENTORY_SCHEMA, Inventory};

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
schema = "l2r.architecture/v0"

[system]
id = "s"
name = "S"
rust_crate = "s_scaffold"
lean_inventory = "inventory.json"
output_dir = "out"

[[component]]
id = "b"
name = "B"
profile = "composite/v0"
responsibility = "r"
rust_module = "b"

[[component]]
id = "a"
name = "A"
parent = "b"
profile = "composite/v0"
responsibility = "r"
rust_module = "a"
"#;

    #[test]
    fn unknown_fields_are_rejected_not_ignored() {
        let typo = MINIMAL.replace(
            "responsibility = \"r\"\nrust_module = \"a\"",
            "responsiblity = \"r\"\nrust_module = \"a\"",
        );
        let err = load::parse_model(&typo).unwrap_err();
        assert!(err.contains("responsiblity"), "{err}");
    }

    #[test]
    fn canonical_form_ignores_declaration_order() {
        let model = load::parse_model(MINIMAL).unwrap();
        let mut reversed = model.clone();
        reversed.components.reverse();
        assert_ne!(model, reversed);
        assert_eq!(
            digest::digest_of(&model.canonical()),
            digest::digest_of(&reversed.canonical())
        );
    }

    #[test]
    fn children_are_returned_in_canonical_order() {
        let model = load::parse_model(MINIMAL).unwrap();
        let ids: Vec<_> = model.children("b").iter().map(|c| c.id.clone()).collect();
        assert_eq!(ids, vec!["a"]);
    }
}
