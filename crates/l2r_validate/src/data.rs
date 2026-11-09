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

use l2r_model::{DeclKind, Inventory};

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

/// Resolves every type in `uses` and everything their fields name.
pub fn resolve(
    uses: &[TypeUse<'_>],
    inventory: &Inventory,
    findings: &mut Findings,
) -> BTreeMap<String, DataType> {
    let mut resolved: BTreeMap<String, DataType> = BTreeMap::new();
    let mut failed: BTreeSet<String> = BTreeSet::new();
    let mut queue: Vec<(String, String, String)> = uses
        .iter()
        .map(|u| {
            (
                u.subject.clone(),
                u.role.to_string(),
                u.lean_name.to_string(),
            )
        })
        .collect();
    while let Some((subject, role, name)) = queue.pop() {
        if resolved.contains_key(&name) || failed.contains(&name) {
            continue;
        }
        match resolve_one(&subject, &role, &name, inventory, findings) {
            Some(data) => {
                for dependency in data.dependencies() {
                    queue.push((
                        subject.clone(),
                        format!("field type of `{name}`"),
                        dependency.to_string(),
                    ));
                }
                resolved.insert(name, data);
            }
            None => {
                failed.insert(name);
            }
        }
    }
    check_unique_paths(&resolved, findings);
    check_acyclic(&resolved, findings);
    resolved
}

fn resolve_one(
    subject: &str,
    role: &str,
    name: &str,
    inventory: &Inventory,
    findings: &mut Findings,
) -> Option<DataType> {
    if !names::is_lean_name(name) {
        findings.error(
            "L2R-L002",
            subject,
            format!("{role} `{name}` is not a qualified Lean name"),
        );
        return None;
    }
    let Some(decl) = inventory.find(name) else {
        findings.error(
            "L2R-L003",
            subject,
            format!("{role} `{name}` is not in the Lean inventory"),
        );
        return None;
    };
    if decl.kind != DeclKind::Inductive {
        findings.error(
            "L2R-L004",
            subject,
            format!(
                "{role} `{name}` must be a Lean inductive type or structure, but the inventory records a {:?}",
                decl.kind
            ),
        );
        return None;
    }
    let mut unsupported = |why: String| {
        findings.error("L2R-L005", subject, format!("{role} `{name}` {why}"));
    };
    if decl.type_ != "Type" || decl.params != 0 || decl.indices != 0 {
        unsupported("must be a plain `Type` without parameters or indices".into());
        return None;
    }
    if decl.constructors.is_empty() {
        unsupported("has no constructor".into());
        return None;
    }
    if decl
        .constructors
        .iter()
        .any(|c| c.fields.len() != c.arity as usize)
    {
        unsupported("lists fewer fields than its constructors have (inventory v1 required)".into());
        return None;
    }
    let mut field_type = |text: &str| -> Option<FieldType> {
        let parsed = parse_field_type(text);
        if parsed.is_none() {
            unsupported(format!(
                "has a field of type `{text}`; supported: Bool, UInt8, UInt16, UInt32, UInt64, \
                 another supported type, or an Option of one of these"
            ));
        }
        parsed
    };
    let (path, last) = match name.rsplit_once('.') {
        Some((path, last)) => (path.split('.').collect::<Vec<_>>(), last),
        None => (Vec::new(), name),
    };
    let modules: Option<Vec<String>> = path.into_iter().map(rust_identifier).collect();
    let rust_name = names::upper_camel(last);
    let shape = if decl.structure && decl.constructors.len() == 1 {
        let ctor = &decl.constructors[0];
        let mut fields = Vec::new();
        for field in &ctor.fields {
            fields.push((field.name.clone(), field_type(&field.type_)?));
        }
        let mut rust_names = BTreeSet::new();
        let mut data_fields = Vec::new();
        for (lean_name, ty) in fields {
            let Some(rust_name) =
                rust_identifier(&lean_name).filter(|n| rust_names.insert(n.clone()))
            else {
                findings.error(
                    "L2R-L007",
                    subject,
                    format!("field `{lean_name}` of {role} `{name}` cannot be given a unique Rust field name"),
                );
                return None;
            };
            data_fields.push(DataField {
                lean_name,
                rust_name,
                ty,
            });
        }
        Shape::Struct {
            constructor: ctor.name.clone(),
            fields: data_fields,
        }
    } else {
        let mut parsed = Vec::new();
        for ctor in &decl.constructors {
            let mut fields = Vec::new();
            for field in &ctor.fields {
                fields.push(field_type(&field.type_)?);
            }
            parsed.push((ctor.name.clone(), fields));
        }
        let mut rust_names = BTreeSet::new();
        let mut variants = Vec::new();
        for (lean_name, fields) in parsed {
            let Some(rust_name) = names::upper_camel(names::lean_last_segment(&lean_name))
                .filter(|n| rust_names.insert(n.clone()))
            else {
                findings.error(
                    "L2R-L007",
                    subject,
                    format!("constructor `{lean_name}` of {role} cannot be given a unique Rust variant name"),
                );
                return None;
            };
            variants.push(DataVariant {
                lean_name,
                rust_name,
                fields,
            });
        }
        Shape::Enum(variants)
    };
    let (Some(modules), Some(rust_name)) = (modules, rust_name) else {
        findings.error(
            "L2R-L007",
            subject,
            format!("{role} `{name}` cannot be given a Rust module path and type name"),
        );
        return None;
    };
    Some(DataType {
        lean_name: name.to_string(),
        modules,
        rust_name,
        shape,
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

fn parse_atom(text: &str) -> Option<FieldType> {
    Some(match text {
        "Bool" => FieldType::Bool,
        "UInt8" => FieldType::U8,
        "UInt16" => FieldType::U16,
        "UInt32" => FieldType::U32,
        "UInt64" => FieldType::U64,
        // Lean types L2R does not mirror; named so the message is precise.
        "Nat" | "Int" | "String" | "Char" | "Float" | "Unit" | "USize" => return None,
        name if names::is_lean_name(name) => FieldType::Named(name.to_string()),
        _ => return None,
    })
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

/// A recursive type has no finite `Copy` representation.
fn check_acyclic(resolved: &BTreeMap<String, DataType>, findings: &mut Findings) {
    for start in resolved.keys() {
        let mut stack: Vec<&str> = resolved[start].dependencies();
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        while let Some(name) = stack.pop() {
            if name == start {
                findings.error(
                    "L2R-L008",
                    format!("type `{start}`"),
                    "is recursive; only finite, non-recursive data types are supported",
                );
                break;
            }
            if visited.insert(name)
                && let Some(data) = resolved.get(name)
            {
                stack.extend(data.dependencies());
            }
        }
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
