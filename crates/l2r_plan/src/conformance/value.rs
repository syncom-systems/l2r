//! Concrete values of the data-type subset (inductives, structures, `Bool`,
//! `UIntN`, `Option`): which ones a harness samples, and how one is written as
//! a Rust and as a Lean expression.

use l2r_validate::data::{DataField, DataType, DataVariant, FieldType, Shape};

use super::{Types, missing};
use crate::PlanError;

/// A concrete value of a data type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Value {
    Bool(bool),
    UInt(u64),
    None,
    Some(Box<Value>),
    /// Constructor `index` of the Lean type `ty`, applied to `args`.
    Data {
        ty: String,
        index: usize,
        args: Vec<Value>,
    },
}

fn resolved<'a>(name: &str, types: &'a Types) -> Result<&'a DataType, PlanError> {
    types.get(name).ok_or_else(|| missing(name))
}

/// Sample values of a field type: for an integer the smallest values, the
/// middle and the largest; for data, see [`data_samples`].
fn samples(ty: &FieldType, types: &Types) -> Result<Vec<Value>, PlanError> {
    Ok(match ty {
        FieldType::Bool => vec![Value::Bool(false), Value::Bool(true)],
        FieldType::U8 => unsigned_samples(8),
        FieldType::U16 => unsigned_samples(16),
        FieldType::U32 => unsigned_samples(32),
        FieldType::U64 => unsigned_samples(64),
        FieldType::Option(inner) => optional_samples(inner, types)?,
        FieldType::Named(name) => data_samples(name, types)?,
    })
}

fn unsigned_samples(bits: u32) -> Vec<Value> {
    let max = u64::MAX >> (64 - bits);
    [0, 1, max / 2 + 1, max]
        .into_iter()
        .map(Value::UInt)
        .collect()
}

fn optional_samples(inner: &FieldType, types: &Types) -> Result<Vec<Value>, PlanError> {
    let present = samples(inner, types)?
        .into_iter()
        .map(|v| Value::Some(Box::new(v)));
    Ok(std::iter::once(Value::None).chain(present).collect())
}

/// Sample values of the data type `name`: every constructor, each field taking
/// each of its sample values once while the others hold their first.
pub(super) fn data_samples(name: &str, types: &Types) -> Result<Vec<Value>, PlanError> {
    let data = resolved(name, types)?;
    // Per constructor, then interleaved, so that a cap keeps every constructor.
    let per_constructor = constructor_fields(data)
        .iter()
        .enumerate()
        .map(|(index, fields)| constructor_samples(name, index, fields, types))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(interleaved(&per_constructor))
}

/// The field types of every constructor: one per variant of an inductive, the
/// single one of a structure.
fn constructor_fields(data: &DataType) -> Vec<Vec<&FieldType>> {
    match &data.shape {
        Shape::Enum(variants) => variants.iter().map(|v| v.fields.iter().collect()).collect(),
        Shape::Struct { fields, .. } => vec![fields.iter().map(|f| &f.ty).collect()],
    }
}

/// The distinct samples of constructor `index` of `ty`: every field at its
/// first sample, then each field through its samples in turn, then every field
/// at its last.
fn constructor_samples(
    ty: &str,
    index: usize,
    fields: &[&FieldType],
    types: &Types,
) -> Result<Vec<Value>, PlanError> {
    let lists = fields
        .iter()
        .map(|f| samples(f, types))
        .collect::<Result<Vec<_>, _>>()?;
    let first: Vec<Value> = lists.iter().map(|l| l[0].clone()).collect();
    let last: Vec<Value> = lists.iter().map(|l| l[l.len() - 1].clone()).collect();
    let mut arguments = vec![first.clone()];
    for (i, list) in lists.iter().enumerate() {
        for v in list {
            let mut args = first.clone();
            args[i] = v.clone();
            arguments.push(args);
        }
    }
    arguments.push(last);
    let values = arguments.into_iter().map(|args| Value::Data {
        ty: ty.to_string(),
        index,
        args,
    });
    Ok(distinct(values))
}

/// The values in order of first occurrence.
fn distinct(values: impl Iterator<Item = Value>) -> Vec<Value> {
    let mut unique: Vec<Value> = Vec::new();
    for v in values {
        if !unique.contains(&v) {
            unique.push(v);
        }
    }
    unique
}

/// The first of every list, then the second of every list that has one, and so
/// on.
fn interleaved(lists: &[Vec<Value>]) -> Vec<Value> {
    let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
    (0..longest)
        .flat_map(|i| lists.iter().filter_map(move |list| list.get(i).cloned()))
        .collect()
}

impl Value {
    /// The value as an expression of the generated crate `krate`.
    pub(super) fn rust_expr(&self, types: &Types, krate: &str) -> Result<String, PlanError> {
        Ok(match self {
            Value::Bool(b) => b.to_string(),
            Value::UInt(n) => n.to_string(),
            Value::None => "None".into(),
            Value::Some(inner) => format!("Some({})", inner.rust_expr(types, krate)?),
            Value::Data { ty, index, args } => {
                let data = resolved(ty, types)?;
                let args = args
                    .iter()
                    .map(|a| a.rust_expr(types, krate))
                    .collect::<Result<Vec<_>, _>>()?;
                rust_construction(data, *index, &args, krate)
            }
        })
    }

    /// The value as a Lean term, with its type ascribed.
    pub(super) fn lean_expr(&self, types: &Types) -> Result<String, PlanError> {
        Ok(match self {
            Value::Bool(b) => b.to_string(),
            Value::UInt(n) => n.to_string(),
            Value::None => "none".into(),
            Value::Some(inner) => format!("(some {})", inner.lean_expr(types)?),
            Value::Data { ty, index, args } => {
                let constructor = lean_constructor(resolved(ty, types)?, *index);
                let mut text = format!("(_root_.{constructor}");
                for arg in args {
                    text.push(' ');
                    text.push_str(&arg.lean_expr(types)?);
                }
                text.push_str(&format!(" : _root_.{ty})"));
                text
            }
        })
    }
}

/// `crate_name::types::tank::Level`.
fn rust_path(data: &DataType, krate: &str) -> String {
    data.rust_path().replacen("crate", krate, 1)
}

/// Constructor `index` of `data` applied to `args`, in Rust.
fn rust_construction(data: &DataType, index: usize, args: &[String], krate: &str) -> String {
    let path = rust_path(data, krate);
    match &data.shape {
        Shape::Struct { fields, .. } => rust_struct_literal(&path, fields, args),
        Shape::Enum(variants) => rust_variant(&path, &variants[index], args),
    }
}

fn rust_struct_literal(path: &str, fields: &[DataField], args: &[String]) -> String {
    if fields.is_empty() {
        return format!("{path} {{}}");
    }
    let fields = fields
        .iter()
        .zip(args)
        .map(|(f, a)| format!("{}: {a}", f.rust_name))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{path} {{ {fields} }}")
}

fn rust_variant(path: &str, variant: &DataVariant, args: &[String]) -> String {
    let name = &variant.rust_name;
    if args.is_empty() {
        format!("{path}::{name}")
    } else {
        format!("{path}::{name}({})", args.join(", "))
    }
}

/// The Lean name of constructor `index` of `data`.
fn lean_constructor(data: &DataType, index: usize) -> &str {
    match &data.shape {
        Shape::Struct { constructor, .. } => constructor,
        Shape::Enum(variants) => &variants[index].lean_name,
    }
}
