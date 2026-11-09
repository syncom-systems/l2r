//! `src/types.rs`: one Rust type for every Lean data type the model uses.
//!
//! The module tree mirrors the Lean namespaces, so a Lean type has exactly one
//! Rust type whichever machines, ports and routes mention it. Every type is
//! plain `Copy` data. For the generated contract tests each type also has a
//! `samples()` function returning representative values: every constructor,
//! and every boundary value of every field at least once (the other fields
//! held at their first value), plus every field at its last value. Their number
//! is the sum, not the product, of the fields' sample counts, so wide
//! structures stay cheap. Those test-only blocks are marked
//! `#[rustfmt::skip]`: their layout depends on the lengths of the user's Lean
//! names, and a generated file must not fail a formatting check.

use std::collections::BTreeMap;

use l2r_validate::data::{DataType, FieldType, Shape};

use crate::plan::{Artifact, Ownership};
use crate::text;
use crate::v2::{Gen, crate_root::PROFILE};
use crate::{PlanError, planned_path};

/// Name of the helper module with the boundary values. A Lean namespace can
/// never map to it: a mapped module name does not end in an underscore.
const SAMPLES_MODULE: &str = "samples_";

#[derive(Default)]
struct Node<'a> {
    types: Vec<&'a DataType>,
    children: BTreeMap<&'a str, Node<'a>>,
}

pub fn artifact(generator: &Gen<'_>) -> Result<Artifact, PlanError> {
    let types = generator.validated.types();
    let mut root = Node::default();
    for data in types.values() {
        let mut node = &mut root;
        for module in &data.modules {
            node = node.children.entry(module.as_str()).or_default();
        }
        node.types.push(data);
    }
    let mut out = text::generated_marker(PROFILE);
    out.push('\n');
    out.push_str(&text::inner_doc(
        "Rust mirrors of the Lean data types, one per Lean type, at the path its Lean\n\
         namespace gives. Every type is plain `Copy` data; behavior stays in Lean and\n\
         in the implementation files.",
    ));
    out.push('\n');
    emit(&root, types, 0, &mut out)?;
    out.push_str(SAMPLES);
    Ok(Artifact {
        path: planned_path("src/types.rs".into())?,
        ownership: Ownership::Generated,
        profile: PROFILE.into(),
        purpose: "Rust mirrors of the Lean data types".into(),
        components: vec![generator.root.id.clone()],
        lean: types
            .values()
            .flat_map(|t| {
                let mut names = vec![t.lean_name.clone()];
                match &t.shape {
                    Shape::Enum(variants) => {
                        names.extend(variants.iter().map(|v| v.lean_name.clone()))
                    }
                    Shape::Struct { constructor, .. } => names.push(constructor.clone()),
                }
                names
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect(),
        content: out,
    })
}

const SAMPLES: &str = "/// Boundary values for the generated contract tests.\n\
#[cfg(test)]\n\
pub mod samples_ {\n\
\x20   /// The first value: the one a field holds while another field varies.\n\
\x20   pub fn first<T: Copy>(values: &[T]) -> T {\n\
\x20       values[0]\n\
\x20   }\n\
\n\
\x20   /// The last value: for an integer, the largest.\n\
\x20   pub fn last<T: Copy>(values: &[T]) -> T {\n\
\x20       values[values.len() - 1]\n\
\x20   }\n\
\n\
\x20   /// `None`, then every value.\n\
\x20   pub fn options<T>(values: Vec<T>) -> Vec<Option<T>> {\n\
\x20       let mut out = vec![None];\n\
\x20       out.extend(values.into_iter().map(Some));\n\
\x20       out\n\
\x20   }\n\
\n\
\x20   /// Both booleans.\n\
\x20   pub fn bools() -> Vec<bool> {\n\
\x20       vec![false, true]\n\
\x20   }\n\
\n\
\x20   /// The smallest values and the largest.\n\
\x20   pub fn u8s() -> Vec<u8> {\n\
\x20       vec![0, 1, u8::MAX]\n\
\x20   }\n\
\n\
\x20   /// The smallest values and the largest.\n\
\x20   pub fn u16s() -> Vec<u16> {\n\
\x20       vec![0, 1, u16::MAX]\n\
\x20   }\n\
\n\
\x20   /// The smallest values and the largest.\n\
\x20   pub fn u32s() -> Vec<u32> {\n\
\x20       vec![0, 1, u32::MAX]\n\
\x20   }\n\
\n\
\x20   /// The smallest values and the largest.\n\
\x20   pub fn u64s() -> Vec<u64> {\n\
\x20       vec![0, 1, u64::MAX]\n\
\x20   }\n\
}\n";

fn emit(
    node: &Node<'_>,
    types: &BTreeMap<String, DataType>,
    depth: usize,
    out: &mut String,
) -> Result<(), PlanError> {
    let pad = "    ".repeat(depth);
    let mut sorted = node.types.clone();
    sorted.sort_by(|a, b| a.rust_name.cmp(&b.rust_name));
    for data in sorted {
        out.push_str(&type_def(data, types, &pad)?);
        out.push('\n');
    }
    for (module, child) in &node.children {
        out.push_str(&format!("{pad}pub mod {module} {{\n"));
        let mut body = String::new();
        emit(child, types, depth + 1, &mut body)?;
        body.truncate(body.trim_end().len());
        out.push_str(&body);
        out.push_str(&format!("\n{pad}}}\n\n"));
    }
    Ok(())
}

fn rust_type(ty: &FieldType, types: &BTreeMap<String, DataType>) -> Result<String, PlanError> {
    Ok(match ty {
        FieldType::Bool => "bool".into(),
        FieldType::U8 => "u8".into(),
        FieldType::U16 => "u16".into(),
        FieldType::U32 => "u32".into(),
        FieldType::U64 => "u64".into(),
        FieldType::Named(name) => types
            .get(name)
            .map(DataType::rust_path)
            .ok_or_else(|| PlanError(format!("type `{name}` was not resolved")))?,
        FieldType::Option(inner) => format!("Option<{}>", rust_type(inner, types)?),
    })
}

/// The expression giving the sample values of a field type.
fn samples_of(ty: &FieldType, types: &BTreeMap<String, DataType>) -> Result<String, PlanError> {
    let helper = |name: &str| format!("crate::types::{SAMPLES_MODULE}::{name}");
    Ok(match ty {
        FieldType::Bool => format!("{}()", helper("bools")),
        FieldType::U8 => format!("{}()", helper("u8s")),
        FieldType::U16 => format!("{}()", helper("u16s")),
        FieldType::U32 => format!("{}()", helper("u32s")),
        FieldType::U64 => format!("{}()", helper("u64s")),
        FieldType::Named(_) => format!("{}::samples()", rust_type(ty, types)?),
        FieldType::Option(inner) => {
            format!("{}({})", helper("options"), samples_of(inner, types)?)
        }
    })
}

/// A constructor expression prefix, its field types, and (for a structure) its
/// field names.
type Constructor<'a> = (String, Vec<&'a FieldType>, Option<Vec<&'a str>>);

fn type_def(
    data: &DataType,
    types: &BTreeMap<String, DataType>,
    pad: &str,
) -> Result<String, PlanError> {
    const DERIVE: &str = "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]";
    let name = &data.rust_name;
    let mut out = String::new();
    let mut constructors: Vec<Constructor<'_>> = Vec::new();
    match &data.shape {
        Shape::Enum(variants) => {
            out.push_str(&format!(
                "{pad}/// Mirrors `{}`.\n{pad}{DERIVE}\n{pad}pub enum {name} {{\n",
                data.lean_name
            ));
            for variant in variants {
                out.push_str(&format!("{pad}    /// `{}`\n", variant.lean_name));
                if variant.fields.is_empty() {
                    out.push_str(&format!("{pad}    {},\n", variant.rust_name));
                } else {
                    let fields = variant
                        .fields
                        .iter()
                        .map(|f| rust_type(f, types))
                        .collect::<Result<Vec<_>, _>>()?;
                    let line = format!("{pad}    {}({}),\n", variant.rust_name, fields.join(", "));
                    if line.trim_end().len() <= 100 {
                        out.push_str(&line);
                    } else {
                        out.push_str(&format!("{pad}    {}(\n", variant.rust_name));
                        for field in fields {
                            out.push_str(&format!("{pad}        {field},\n"));
                        }
                        out.push_str(&format!("{pad}    ),\n"));
                    }
                }
                constructors.push((
                    format!("{name}::{}", variant.rust_name),
                    variant.fields.iter().collect(),
                    None,
                ));
            }
            out.push_str(&format!("{pad}}}\n"));
        }
        Shape::Struct {
            constructor,
            fields,
        } => {
            out.push_str(&format!(
                "{pad}/// Mirrors `{}` (constructor `{constructor}`).\n{pad}{DERIVE}\n",
                data.lean_name
            ));
            if fields.is_empty() {
                out.push_str(&format!("{pad}pub struct {name} {{}}\n"));
            } else {
                out.push_str(&format!("{pad}pub struct {name} {{\n"));
                for field in fields {
                    out.push_str(&format!(
                        "{pad}    /// `{}`\n{pad}    pub {}: {},\n",
                        field.lean_name,
                        field.rust_name,
                        rust_type(&field.ty, types)?
                    ));
                }
                out.push_str(&format!("{pad}}}\n"));
            }
            constructors.push((
                name.clone(),
                fields.iter().map(|f| &f.ty).collect(),
                Some(fields.iter().map(|f| f.rust_name.as_str()).collect()),
            ));
        }
    }
    // Representative values, for the generated contract tests. Each field
    // takes every one of its sample values once while the others hold their
    // first value, and one more value has every field at its last (for an
    // integer, its largest) value. The number of values is the sum, not the
    // product, of the fields' sample counts.
    out.push_str(&format!(
        "\n{pad}#[cfg(test)]\n{pad}#[rustfmt::skip]\n{pad}impl {name} {{\n\
         {pad}    /// Representative values: every constructor; every sample value of every\n\
         {pad}    /// field (for an integer 0, 1 and the largest) while the other fields hold\n\
         {pad}    /// their first value; and every field at its last value. Not exhaustive.\n\
         {pad}    #[allow(clippy::vec_init_then_push)]\n\
         {pad}    pub fn samples() -> Vec<Self> {{\n\
         {pad}        let mut out = Vec::new();\n"
    ));
    let helper = format!("crate::types::{SAMPLES_MODULE}");
    for (constructor, fields, field_names) in &constructors {
        let value = |args: &[String]| match field_names {
            None if args.is_empty() => constructor.clone(),
            None => format!("{constructor}({})", args.join(", ")),
            Some(_) if args.is_empty() => format!("{constructor} {{}}"),
            Some(names) => format!(
                "{constructor} {{ {} }}",
                names
                    .iter()
                    .zip(args)
                    .map(|(n, a)| format!("{n}: {a}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        if fields.is_empty() {
            out.push_str(&format!("{pad}        out.push({});\n", value(&[])));
            continue;
        }
        out.push_str(&format!("{pad}        {{\n"));
        for (i, field) in fields.iter().enumerate() {
            out.push_str(&format!(
                "{pad}            let s{i} = {};\n",
                samples_of(field, types)?
            ));
        }
        for i in 0..fields.len() {
            let args: Vec<String> = (0..fields.len())
                .map(|j| {
                    if i == j {
                        "v".to_string()
                    } else {
                        format!("{helper}::first(&s{j})")
                    }
                })
                .collect();
            out.push_str(&format!(
                "{pad}            for &v in &s{i} {{\n{pad}                out.push({});\n{pad}            }}\n",
                value(&args)
            ));
        }
        let last: Vec<String> = (0..fields.len())
            .map(|j| format!("{helper}::last(&s{j})"))
            .collect();
        out.push_str(&format!(
            "{pad}            out.push({});\n{pad}        }}\n",
            value(&last)
        ));
    }
    out.push_str(&format!("{pad}        out\n{pad}    }}\n{pad}}}\n"));
    Ok(out)
}
