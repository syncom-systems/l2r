//! The Lean renderers of the replayer: one function per data type that prints
//! a value as Rust's `Debug` prints its mirror, so that the two sides'
//! observations compare as text.

use l2r_validate::data::{DataField, DataType, DataVariant, FieldType, Shape};

use super::{Types, missing};
use crate::PlanError;

/// `showOption` and `showList`, which the per-type renderers build on.
const GENERIC_RENDERERS: &str = r#"def showOption {α : Type} (f : α → String) : Option α → String
  | none => "None"
  | some x => "Some(" ++ f x ++ ")"

def showList {α : Type} (f : α → String) (xs : List α) : String :=
  "[" ++ ";".intercalate (xs.map f) ++ "]"

"#;

/// Lean for `", "` between two rendered fields.
const COMMA: &str = " ++ \", \" ++ ";

/// Every data type of a model with the name of its renderer, in dependency
/// order, so that each renderer is defined before it is used.
pub(super) struct Renderers<'a> {
    named: Vec<(&'a DataType, String)>,
}

impl<'a> Renderers<'a> {
    pub(super) fn new(types: &'a Types) -> Self {
        let mut order: Vec<&DataType> = Vec::new();
        for data in types.values() {
            push_after_dependencies(data, types, &mut order);
        }
        let named = order
            .into_iter()
            .enumerate()
            .map(|(i, d)| (d, format!("show{i}")))
            .collect();
        Self { named }
    }

    /// The name of the renderer of the Lean type `ty`.
    pub(super) fn name(&self, ty: &str) -> Result<&str, PlanError> {
        self.named
            .iter()
            .find(|(d, _)| d.lean_name == ty)
            .map(|(_, name)| name.as_str())
            .ok_or_else(|| missing(ty))
    }

    /// The Lean function that renders a field of type `ty`.
    fn show(&self, ty: &FieldType) -> Result<String, PlanError> {
        Ok(match ty {
            FieldType::Named(name) => self.name(name)?.to_string(),
            FieldType::Option(inner) => format!("(showOption {})", self.show(inner)?),
            _ => "toString".into(),
        })
    }

    /// The generic renderers, then the definition of every data type's.
    pub(super) fn definitions(&self) -> Result<String, PlanError> {
        let mut out = String::from(GENERIC_RENDERERS);
        for (data, name) in &self.named {
            out.push_str(&self.definition(data, name)?);
        }
        Ok(out)
    }

    fn definition(&self, data: &DataType, name: &str) -> Result<String, PlanError> {
        let cases = match &data.shape {
            Shape::Struct { fields, .. } => self.structure_case(&data.rust_name, fields)?,
            Shape::Enum(variants) => variants
                .iter()
                .map(|v| self.variant_case(v))
                .collect::<Result<String, _>>()?,
        };
        Ok(format!(
            "def {name} : _root_.{} → String\n{cases}\n",
            data.lean_name
        ))
    }

    /// The single case of a structure: `Name { field: value, .. }`.
    fn structure_case(&self, rust_name: &str, fields: &[DataField]) -> Result<String, PlanError> {
        let binders: Vec<String> = (0..fields.len()).map(|i| format!("a{i}")).collect();
        let parts = fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                Ok(format!(
                    "\"{}: \" ++ {} a{i}",
                    f.rust_name,
                    self.show(&f.ty)?
                ))
            })
            .collect::<Result<Vec<_>, PlanError>>()?;
        let body = if parts.is_empty() {
            format!("\"{rust_name}\"")
        } else {
            format!("\"{rust_name} {{ \" ++ {} ++ \" }}\"", parts.join(COMMA))
        };
        Ok(format!("  | ⟨{}⟩ => {body}\n", binders.join(", ")))
    }

    /// The case of one variant of an inductive: `Name(value, ..)`.
    fn variant_case(&self, variant: &DataVariant) -> Result<String, PlanError> {
        let binders: Vec<String> = (0..variant.fields.len())
            .map(|i| format!(" a{i}"))
            .collect();
        let parts = variant
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| Ok(format!("{} a{i}", self.show(f)?)))
            .collect::<Result<Vec<_>, PlanError>>()?;
        let body = if parts.is_empty() {
            format!("\"{}\"", variant.rust_name)
        } else {
            format!(
                "\"{}(\" ++ {} ++ \")\"",
                variant.rust_name,
                parts.join(COMMA)
            )
        };
        Ok(format!(
            "  | _root_.{}{} => {body}\n",
            variant.lean_name,
            binders.concat()
        ))
    }
}

/// Appends `data` to `order` after the data types it depends on, unless it is
/// already there.
fn push_after_dependencies<'a>(
    data: &'a DataType,
    types: &'a Types,
    order: &mut Vec<&'a DataType>,
) {
    if order.iter().any(|d| d.lean_name == data.lean_name) {
        return;
    }
    for dependency in data.dependencies() {
        if let Some(d) = types.get(dependency) {
            push_after_dependencies(d, types, order);
        }
    }
    order.push(data);
}
