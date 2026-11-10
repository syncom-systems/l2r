//! Data types of model schema `v2`: the Lean types a machine or port may use,
//! and the Rust type generated for each.
//!
//! The supported subset is deliberately small and closed: a non-recursive Lean
//! inductive type or structure in `Type`, without parameters or indices, whose
//! fields are `Bool`, a fixed-width unsigned integer, another supported type,
//! or an `Option` of one of those. Everything else is rejected, never
//! approximated. Each Lean type maps to exactly one Rust type, in the generated
//! `types` module, at the path its Lean namespace gives.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;

use l2r_model::{Constructor, DeclKind, Declaration, Field, Inventory};

use crate::diagnostic::Findings;
use crate::names;

/// The type of one field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    /// Lean `Bool`, Rust `bool`.
    Bool,
    /// Lean `UInt8`, Rust `u8`.
    U8,
    /// Lean `UInt16`, Rust `u16`.
    U16,
    /// Lean `UInt32`, Rust `u32`.
    U32,
    /// Lean `UInt64`, Rust `u64`.
    U64,
    /// Another supported Lean type, by qualified name.
    Named(String),
    /// Lean `Option T`, Rust `Option<T>`; `T` is never itself an option.
    Option(Box<FieldType>),
}

/// A Lean data type and the Rust type that mirrors it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataType {
    /// Qualified Lean name.
    pub lean_name: String,
    /// Rust modules below `crate::types`, from the Lean namespace.
    pub modules: Vec<String>,
    /// Rust type name.
    pub rust_name: String,
    /// Constructors and fields.
    pub shape: Shape,
}

/// The two shapes a data type can have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// A Lean `inductive`: a Rust `enum` with positional fields.
    Enum(Vec<DataVariant>),
    /// A Lean `structure`: a Rust `struct` with named fields.
    Struct {
        /// Qualified name of the structure's constructor.
        constructor: String,
        /// Fields, in declaration order.
        fields: Vec<DataField>,
    },
}

/// One constructor of an inductive type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    /// Qualified Lean constructor name.
    pub lean_name: String,
    /// Rust variant name.
    pub rust_name: String,
    /// Field types, in order.
    pub fields: Vec<FieldType>,
}

/// One field of a structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    /// Lean field name.
    pub lean_name: String,
    /// Rust field name.
    pub rust_name: String,
    /// Field type.
    pub ty: FieldType,
}

impl DataType {
    /// The Rust path of the type from the crate root, e.g.
    /// `crate::types::tank::Level`.
    pub fn rust_path(&self) -> String {
        let mut path = String::from("crate::types");
        for module in &self.modules {
            path.push_str("::");
            path.push_str(module);
        }
        path.push_str("::");
        path.push_str(&self.rust_name);
        path
    }

    /// The other data types this one's fields name.
    pub fn dependencies(&self) -> Vec<&str> {
        let fields: Vec<&FieldType> = match &self.shape {
            Shape::Enum(variants) => variants.iter().flat_map(|v| v.fields.iter()).collect(),
            Shape::Struct { fields, .. } => fields.iter().map(|f| &f.ty).collect(),
        };
        fields.into_iter().filter_map(FieldType::named).collect()
    }
}

impl FieldType {
    /// The Lean type this field names, looking through `Option`.
    pub fn named(&self) -> Option<&str> {
        match self {
            FieldType::Named(name) => Some(name),
            FieldType::Option(inner) => inner.named(),
            _ => None,
        }
    }
}

/// Rust prelude types: a mirror of that name would capture the generated
/// code's own `Option<..>` and `Vec<..>`, and confuse every implementer.
const PRELUDE_TYPES: &[&str] = &["Box", "Option", "Result", "String", "Vec"];

/// `fillLevel` → `fill_level`, `CrossingV1` → `crossing_v1`.
pub fn snake_case(segment: &str) -> String {
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for c in segment.chars() {
        if c.is_ascii_uppercase() {
            if previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit()) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
        previous = Some(c);
    }
    out
}

fn rust_identifier(segment: &str) -> Option<String> {
    let name = snake_case(segment);
    (names::is_identifier(&name) && !names::is_rust_keyword(&name)).then_some(name)
}

/// One use of a Lean type by the model: who names it, and in which role.
pub struct TypeUse<'a> {
    /// The model element, for diagnostics.
    pub subject: String,
    /// The role of the type in that element.
    pub role: &'a str,
    /// The qualified Lean name.
    pub lean_name: &'a str,
}

/// A Lean type still to resolve, on behalf of the model element that led to
/// it: by naming it, or through a field of a type it names.
struct Wanted {
    subject: String,
    role: String,
    lean_name: String,
}

/// Why a wanted type has no Rust mirror: the rule it breaks, and the message.
struct Rejection {
    code: &'static str,
    message: String,
}

impl Wanted {
    fn by(type_use: &TypeUse<'_>) -> Wanted {
        Wanted {
            subject: type_use.subject.clone(),
            role: type_use.role.to_string(),
            lean_name: type_use.lean_name.to_string(),
        }
    }

    /// The types the fields of `data` (the mirror of this type) name.
    fn fields_of(&self, data: &DataType) -> Vec<Wanted> {
        let role = format!("field type of `{}`", self.lean_name);
        let wanted = |dependency: &str| Wanted {
            subject: self.subject.clone(),
            role: role.clone(),
            lean_name: dependency.to_string(),
        };
        data.dependencies().into_iter().map(wanted).collect()
    }

    /// A rejection that says, after the role and name of the type, `why`.
    fn rejected(&self, code: &'static str, why: impl Display) -> Rejection {
        Rejection {
            code,
            message: format!("{} `{}` {why}", self.role, self.lean_name),
        }
    }

    /// The type is outside the supported subset.
    fn unsupported(&self, why: impl Display) -> Rejection {
        self.rejected("L2R-L005", why)
    }
}

/// The outcome for every type met so far. Each type is resolved or rejected
/// once, for the first use that reaches it.
#[derive(Default)]
struct Resolution {
    resolved: BTreeMap<String, DataType>,
    failed: BTreeSet<String>,
}

impl Resolution {
    fn is_settled(&self, lean_name: &str) -> bool {
        self.resolved.contains_key(lean_name) || self.failed.contains(lean_name)
    }

    /// Resolves or rejects `wanted`; returns the types its fields name.
    fn settle(
        &mut self,
        wanted: Wanted,
        inventory: &Inventory,
        findings: &mut Findings,
    ) -> Vec<Wanted> {
        match mirror(&wanted, inventory) {
            Ok(data) => {
                let fields = wanted.fields_of(&data);
                self.resolved.insert(wanted.lean_name, data);
                fields
            }
            Err(rejection) => {
                findings.error(rejection.code, wanted.subject, rejection.message);
                self.failed.insert(wanted.lean_name);
                Vec::new()
            }
        }
    }
}

/// Resolves every type in `uses` and everything their fields name.
pub fn resolve(
    uses: &[TypeUse<'_>],
    inventory: &Inventory,
    findings: &mut Findings,
) -> BTreeMap<String, DataType> {
    let mut resolution = Resolution::default();
    let mut queue: Vec<Wanted> = uses.iter().map(Wanted::by).collect();
    while let Some(wanted) = queue.pop() {
        if !resolution.is_settled(&wanted.lean_name) {
            queue.extend(resolution.settle(wanted, inventory, findings));
        }
    }
    let resolved = resolution.resolved;
    check_unique_paths(&resolved, findings);
    check_acyclic(&resolved, findings);
    resolved
}

/// The Rust mirror of the Lean type `wanted` names.
fn mirror(wanted: &Wanted, inventory: &Inventory) -> Result<DataType, Rejection> {
    let decl = inductive(wanted, inventory)?;
    check_plain_data(wanted, decl)?;
    let shape = shape(wanted, decl)?;
    let (modules, rust_name) = rust_location(wanted)?;
    Ok(DataType {
        lean_name: wanted.lean_name.clone(),
        modules,
        rust_name,
        shape,
    })
}

/// The inventory's declaration of the wanted type: an inductive type or
/// structure.
fn inductive<'a>(wanted: &Wanted, inventory: &'a Inventory) -> Result<&'a Declaration, Rejection> {
    if !names::is_lean_name(&wanted.lean_name) {
        return Err(wanted.rejected("L2R-L002", "is not a qualified Lean name"));
    }
    let Some(decl) = inventory.find(&wanted.lean_name) else {
        return Err(wanted.rejected("L2R-L003", "is not in the Lean inventory"));
    };
    if decl.kind != DeclKind::Inductive {
        let why = format!(
            "must be a Lean inductive type or structure, but the inventory records a {:?}",
            decl.kind
        );
        return Err(wanted.rejected("L2R-L004", why));
    }
    Ok(decl)
}

/// Plain data is a `Type` of its own, with at least one constructor and every
/// constructor field recorded.
fn check_plain_data(wanted: &Wanted, decl: &Declaration) -> Result<(), Rejection> {
    let plain_type = decl.type_ == "Type" && (decl.params, decl.indices) == (0, 0);
    if !plain_type {
        return Err(wanted.unsupported("must be a plain `Type` without parameters or indices"));
    }
    if decl.constructors.is_empty() {
        return Err(wanted.unsupported("has no constructor"));
    }
    let fields_recorded = decl
        .constructors
        .iter()
        .all(|c| c.fields.len() == c.arity as usize);
    if !fields_recorded {
        return Err(wanted
            .unsupported("lists fewer fields than its constructors have (inventory v1 required)"));
    }
    Ok(())
}

/// A Lean `structure` is mirrored as a struct, any other inductive type as an
/// enum.
fn shape(wanted: &Wanted, decl: &Declaration) -> Result<Shape, Rejection> {
    if decl.structure && decl.constructors.len() == 1 {
        struct_shape(wanted, &decl.constructors[0])
    } else {
        enum_shape(wanted, &decl.constructors)
    }
}

/// The types of a constructor's fields, in order.
fn field_types(wanted: &Wanted, constructor: &Constructor) -> Result<Vec<FieldType>, Rejection> {
    let field_type = |field: &Field| {
        parse_field_type(&field.type_).ok_or_else(|| {
            wanted.unsupported(format!(
                "has a field of type `{}`; supported: Bool, UInt8, UInt16, UInt32, UInt64, \
                 another supported type, or an Option of one of these",
                field.type_
            ))
        })
    };
    constructor.fields.iter().map(field_type).collect()
}

fn struct_shape(wanted: &Wanted, constructor: &Constructor) -> Result<Shape, Rejection> {
    let types = field_types(wanted, constructor)?;
    let mut rust_names = BTreeSet::new();
    let mut fields = Vec::new();
    for (field, ty) in constructor.fields.iter().zip(types) {
        let Some(rust_name) = rust_identifier(&field.name).filter(|n| rust_names.insert(n.clone()))
        else {
            return Err(Rejection {
                code: "L2R-L007",
                message: format!(
                    "field `{}` of {} `{}` cannot be given a unique Rust field name",
                    field.name, wanted.role, wanted.lean_name
                ),
            });
        };
        fields.push(DataField {
            lean_name: field.name.clone(),
            rust_name,
            ty,
        });
    }
    Ok(Shape::Struct {
        constructor: constructor.name.clone(),
        fields,
    })
}

fn enum_shape(wanted: &Wanted, constructors: &[Constructor]) -> Result<Shape, Rejection> {
    let types = constructors
        .iter()
        .map(|c| field_types(wanted, c))
        .collect::<Result<Vec<_>, _>>()?;
    let mut rust_names = BTreeSet::new();
    let mut variants = Vec::new();
    for (constructor, fields) in constructors.iter().zip(types) {
        let Some(rust_name) = names::upper_camel(names::lean_last_segment(&constructor.name))
            .filter(|n| rust_names.insert(n.clone()))
        else {
            return Err(Rejection {
                code: "L2R-L007",
                message: format!(
                    "constructor `{}` of {} cannot be given a unique Rust variant name",
                    constructor.name, wanted.role
                ),
            });
        };
        variants.push(DataVariant {
            lean_name: constructor.name.clone(),
            rust_name,
            fields,
        });
    }
    Ok(Shape::Enum(variants))
}

/// The Rust modules and type name that the Lean name of the wanted type gives.
fn rust_location(wanted: &Wanted) -> Result<(Vec<String>, String), Rejection> {
    let name = wanted.lean_name.as_str();
    let (path, last) = match name.rsplit_once('.') {
        Some((path, last)) => (path.split('.').collect::<Vec<_>>(), last),
        None => (Vec::new(), name),
    };
    let modules: Option<Vec<String>> = path.into_iter().map(rust_identifier).collect();
    // Names the generated code and every implementation file use unqualified.
    let rust_name = names::upper_camel(last).filter(|n| !PRELUDE_TYPES.contains(&n.as_str()));
    modules.zip(rust_name).ok_or_else(|| {
        wanted.rejected(
            "L2R-L007",
            format!(
                "cannot be given a Rust module path and type name (a segment is a Rust keyword, or the type would be called {})",
                PRELUDE_TYPES.join(", ")
            ),
        )
    })
}

fn parse_field_type(text: &str) -> Option<FieldType> {
    if let Some(inner) = text.strip_prefix("Option ") {
        return match parse_atom(inner)? {
            FieldType::Option(_) => None,
            atom => Some(FieldType::Option(Box::new(atom))),
        };
    }
    parse_atom(text)
}

/// Lean types L2R does not mirror. They are well-formed Lean names, so they
/// are listed: the finding then says the type is unsupported, not missing.
const UNMIRRORED_TYPES: &[&str] = &["Nat", "Int", "String", "Char", "Float", "Unit", "USize"];

/// The Lean types with a Rust primitive of the same meaning.
fn primitive(text: &str) -> Option<FieldType> {
    match text {
        "Bool" => Some(FieldType::Bool),
        "UInt8" => Some(FieldType::U8),
        "UInt16" => Some(FieldType::U16),
        "UInt32" => Some(FieldType::U32),
        "UInt64" => Some(FieldType::U64),
        _ => None,
    }
}

fn parse_atom(text: &str) -> Option<FieldType> {
    if let Some(primitive) = primitive(text) {
        return Some(primitive);
    }
    let mirrored = !UNMIRRORED_TYPES.contains(&text) && names::is_lean_name(text);
    mirrored.then(|| FieldType::Named(text.to_string()))
}

fn check_unique_paths(resolved: &BTreeMap<String, DataType>, findings: &mut Findings) {
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for data in resolved.values() {
        if let Some(other) = seen.insert(data.rust_path(), &data.lean_name) {
            findings.error(
                "L2R-L007",
                format!("type `{}`", data.lean_name),
                format!(
                    "maps to the same Rust type `{}` as `{other}`",
                    data.rust_path()
                ),
            );
        }
    }
}

/// Every type the fields of `start` lead to, directly or through the fields
/// of other resolved types.
fn types_reached<'a>(
    resolved: &'a BTreeMap<String, DataType>,
    start: &'a DataType,
) -> BTreeSet<&'a str> {
    let mut stack: Vec<&str> = start.dependencies();
    let mut reached: BTreeSet<&str> = BTreeSet::new();
    while let Some(name) = stack.pop() {
        if reached.insert(name) {
            let through = resolved.get(name).map(DataType::dependencies);
            stack.extend(through.unwrap_or_default());
        }
    }
    reached
}

/// A recursive type has no finite `Copy` representation.
fn check_acyclic(resolved: &BTreeMap<String, DataType>, findings: &mut Findings) {
    let recursive = resolved
        .values()
        .filter(|data| types_reached(resolved, data).contains(data.lean_name.as_str()));
    for data in recursive {
        findings.error(
            "L2R-L008",
            format!("type `{}`", data.lean_name),
            "is recursive; only finite, non-recursive data types are supported",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_of_lean_segments() {
        assert_eq!(snake_case("fillLevel"), "fill_level");
        assert_eq!(snake_case("Tank"), "tank");
        assert_eq!(snake_case("CrossingV1"), "crossing_v1");
        assert_eq!(snake_case("raw"), "raw");
        assert_eq!(snake_case("HTTPServer"), "httpserver");
    }

    #[test]
    fn field_types() {
        assert_eq!(parse_field_type("UInt8"), Some(FieldType::U8));
        assert_eq!(
            parse_field_type("Option Tank.Level"),
            Some(FieldType::Option(Box::new(FieldType::Named(
                "Tank.Level".into()
            ))))
        );
        for bad in [
            "Nat",
            "List UInt8",
            "Option (Option Bool)",
            "UInt8 → Bool",
            "",
        ] {
            assert_eq!(parse_field_type(bad), None, "{bad}");
        }
    }
}
