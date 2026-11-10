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

use std::collections::{BTreeMap, BTreeSet};

use l2r_validate::data::{DataField, DataType, DataVariant, FieldType, Shape};

use crate::plan::{Artifact, Ownership};
use crate::text;
use crate::v2::{Gen, crate_root::PROFILE};
use crate::{PlanError, planned_path};

/// Name of the helper module with the boundary values. A Lean namespace can
/// never map to it: a mapped module name does not end in an underscore.
const SAMPLES_MODULE: &str = "samples_";

pub fn artifact(generator: &Gen<'_>) -> Result<Artifact, PlanError> {
    let types = generator.validated.types();
    let mut out = text::generated_marker(PROFILE);
    out.push('\n');
    out.push_str(&text::inner_doc(
        "Rust mirrors of the Lean data types, one per Lean type, at the path its Lean\n\
         namespace gives. Every type is plain `Copy` data; behavior stays in Lean and\n\
         in the implementation files.",
    ));
    out.push('\n');
    out.push_str(&Namespace::of(types).definitions(types, 0)?);
    out.push_str(SAMPLES);
    Ok(Artifact {
        path: planned_path("src/types.rs".into())?,
        ownership: Ownership::Generated,
        profile: PROFILE.into(),
        purpose: "Rust mirrors of the Lean data types".into(),
        components: vec![generator.root.id.clone()],
        lean: lean_names(types),
        content: out,
    })
}

/// Every Lean declaration the mirrors are generated from, once, in order.
fn lean_names(types: &BTreeMap<String, DataType>) -> Vec<String> {
    let names: BTreeSet<String> = types.values().flat_map(declared_names).collect();
    names.into_iter().collect()
}

/// The Lean declarations one mirror is generated from: the type and its
/// constructors.
fn declared_names(data: &DataType) -> Vec<String> {
    let mut names = vec![data.lean_name.clone()];
    match &data.shape {
        Shape::Enum(variants) => names.extend(variants.iter().map(|v| v.lean_name.clone())),
        Shape::Struct { constructor, .. } => names.push(constructor.clone()),
    }
    names
}

/// A Lean namespace as a Rust module: the types declared in it and the
/// namespaces below it.
#[derive(Default)]
struct Namespace<'a> {
    types: Vec<&'a DataType>,
    children: BTreeMap<&'a str, Namespace<'a>>,
}

impl<'a> Namespace<'a> {
    /// The root namespace holding every type at the path its modules give.
    fn of(types: &'a BTreeMap<String, DataType>) -> Self {
        let mut root = Namespace::default();
        for data in types.values() {
            root.below(&data.modules).types.push(data);
        }
        root
    }

    /// The namespace at `modules` below this one, created if it is not there.
    fn below(&mut self, modules: &'a [String]) -> &mut Namespace<'a> {
        let mut namespace = self;
        for module in modules {
            namespace = namespace.children.entry(module.as_str()).or_default();
        }
        namespace
    }

    /// The namespace's types by Rust name, then one `pub mod` per namespace
    /// below it.
    fn definitions(
        &self,
        types: &BTreeMap<String, DataType>,
        depth: usize,
    ) -> Result<String, PlanError> {
        let pad = "    ".repeat(depth);
        let mut sorted = self.types.clone();
        sorted.sort_by(|a, b| a.rust_name.cmp(&b.rust_name));
        let mut out = String::new();
        for data in sorted {
            out.push_str(&type_def(data, types, &pad)?);
            out.push('\n');
        }
        for (module, child) in &self.children {
            let body = child.definitions(types, depth + 1)?;
            out.push_str(&format!(
                "{pad}pub mod {module} {{\n{}\n{pad}}}\n\n",
                body.trim_end()
            ));
        }
        Ok(out)
    }
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

fn rust_type(ty: &FieldType, types: &BTreeMap<String, DataType>) -> Result<String, PlanError> {
    Ok(match ty {
        FieldType::Bool => "bool".into(),
        FieldType::U8 => "u8".into(),
        FieldType::U16 => "u16".into(),
        FieldType::U32 => "u32".into(),
        FieldType::U64 => "u64".into(),
        FieldType::Named(name) => mirror_path(name, types)?,
        FieldType::Option(inner) => format!("Option<{}>", rust_type(inner, types)?),
    })
}

/// The path of the mirror of a Lean data type.
fn mirror_path(name: &str, types: &BTreeMap<String, DataType>) -> Result<String, PlanError> {
    types
        .get(name)
        .map(DataType::rust_path)
        .ok_or_else(|| PlanError(format!("type `{name}` was not resolved")))
}

/// A function of the helper module with the boundary values.
fn helper(name: &str) -> String {
    format!("crate::types::{SAMPLES_MODULE}::{name}")
}

/// The expression giving the sample values of a field type.
fn samples_of(ty: &FieldType, types: &BTreeMap<String, DataType>) -> Result<String, PlanError> {
    Ok(match ty {
        FieldType::Bool => format!("{}()", helper("bools")),
        FieldType::U8 => format!("{}()", helper("u8s")),
        FieldType::U16 => format!("{}()", helper("u16s")),
        FieldType::U32 => format!("{}()", helper("u32s")),
        FieldType::U64 => format!("{}()", helper("u64s")),
        FieldType::Named(name) => format!("{}::samples()", mirror_path(name, types)?),
        FieldType::Option(inner) => {
            format!("{}({})", helper("options"), samples_of(inner, types)?)
        }
    })
}

const DERIVE: &str = "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]";

/// The mirror of one Lean data type: its definition and its `samples()`.
fn type_def(
    data: &DataType,
    types: &BTreeMap<String, DataType>,
    pad: &str,
) -> Result<String, PlanError> {
    let name = &data.rust_name;
    let lean = &data.lean_name;
    let (mirrored, definition) = match &data.shape {
        Shape::Enum(variants) => (format!("`{lean}`"), enum_def(name, variants, types, pad)?),
        Shape::Struct {
            constructor,
            fields,
        } => (
            format!("`{lean}` (constructor `{constructor}`)"),
            struct_def(name, fields, types, pad)?,
        ),
    };
    Ok(format!(
        "{pad}/// Mirrors {mirrored}.\n{pad}{DERIVE}\n{definition}{}",
        samples_impl(data, types, pad)?
    ))
}

/// `pub enum`: one variant per Lean constructor.
fn enum_def(
    name: &str,
    variants: &[DataVariant],
    types: &BTreeMap<String, DataType>,
    pad: &str,
) -> Result<String, PlanError> {
    let mut out = format!("{pad}pub enum {name} {{\n");
    for variant in variants {
        out.push_str(&format!("{pad}    /// `{}`\n", variant.lean_name));
        out.push_str(&variant_def(variant, types, pad)?);
    }
    out.push_str(&format!("{pad}}}\n"));
    Ok(out)
}

/// One variant with its positional fields, laid out as rustfmt would: on one
/// line if it fits in 100 columns, otherwise one field per line.
fn variant_def(
    variant: &DataVariant,
    types: &BTreeMap<String, DataType>,
    pad: &str,
) -> Result<String, PlanError> {
    let name = &variant.rust_name;
    if variant.fields.is_empty() {
        return Ok(format!("{pad}    {name},\n"));
    }
    let fields = variant
        .fields
        .iter()
        .map(|f| rust_type(f, types))
        .collect::<Result<Vec<_>, _>>()?;
    let line = format!("{pad}    {name}({}),\n", fields.join(", "));
    if line.trim_end().len() <= 100 {
        return Ok(line);
    }
    let fields: String = fields
        .iter()
        .map(|field| format!("{pad}        {field},\n"))
        .collect();
    Ok(format!("{pad}    {name}(\n{fields}{pad}    ),\n"))
}

/// `pub struct`: one public field per Lean field.
fn struct_def(
    name: &str,
    fields: &[DataField],
    types: &BTreeMap<String, DataType>,
    pad: &str,
) -> Result<String, PlanError> {
    if fields.is_empty() {
        return Ok(format!("{pad}pub struct {name} {{}}\n"));
    }
    let mut out = format!("{pad}pub struct {name} {{\n");
    for field in fields {
        out.push_str(&format!(
            "{pad}    /// `{}`\n{pad}    pub {}: {},\n",
            field.lean_name,
            field.rust_name,
            rust_type(&field.ty, types)?
        ));
    }
    out.push_str(&format!("{pad}}}\n"));
    Ok(out)
}

/// `samples()`: representative values, for the generated contract tests. Each
/// field takes every one of its sample values once while the others hold their
/// first value, and one more value has every field at its last (for an
/// integer, its largest) value. The number of values is the sum, not the
/// product, of the fields' sample counts.
fn samples_impl(
    data: &DataType,
    types: &BTreeMap<String, DataType>,
    pad: &str,
) -> Result<String, PlanError> {
    let mut out = format!(
        "\n{pad}#[cfg(test)]\n{pad}#[rustfmt::skip]\n{pad}impl {name} {{\n\
         {pad}    /// Representative values: every constructor; every sample value of every\n\
         {pad}    /// field (for an integer 0, 1 and the largest) while the other fields hold\n\
         {pad}    /// their first value; and every field at its last value. Not exhaustive.\n\
         {pad}    #[allow(clippy::vec_init_then_push)]\n\
         {pad}    pub fn samples() -> Vec<Self> {{\n\
         {pad}        let mut out = Vec::new();\n",
        name = data.rust_name
    );
    for constructor in constructors(data) {
        out.push_str(&constructor.samples(types, pad)?);
    }
    out.push_str(&format!("{pad}        out\n{pad}    }}\n{pad}}}\n"));
    Ok(out)
}

/// One way to build a value of a data type: a variant of an enum, or the
/// structure itself.
struct Constructor<'a> {
    /// What the expression starts with, e.g. `Level::Sample` or `Reading`.
    path: String,
    fields: Vec<&'a FieldType>,
    /// The field names of a structure; a variant's fields are positional.
    names: Option<Vec<&'a str>>,
}

fn constructors(data: &DataType) -> Vec<Constructor<'_>> {
    let name = &data.rust_name;
    match &data.shape {
        Shape::Enum(variants) => variants
            .iter()
            .map(|variant| Constructor {
                path: format!("{name}::{}", variant.rust_name),
                fields: variant.fields.iter().collect(),
                names: None,
            })
            .collect(),
        Shape::Struct { fields, .. } => vec![Constructor {
            path: name.clone(),
            fields: fields.iter().map(|f| &f.ty).collect(),
            names: Some(fields.iter().map(|f| f.rust_name.as_str()).collect()),
        }],
    }
}

impl Constructor<'_> {
    /// The expression building a value from one argument per field.
    fn value(&self, args: &[String]) -> String {
        let path = &self.path;
        match &self.names {
            None if args.is_empty() => path.clone(),
            None => format!("{path}({})", args.join(", ")),
            Some(_) if args.is_empty() => format!("{path} {{}}"),
            Some(names) => format!(
                "{path} {{ {} }}",
                names
                    .iter()
                    .zip(args)
                    .map(|(n, a)| format!("{n}: {a}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    /// The statements of `samples()` that push this constructor's values.
    fn samples(&self, types: &BTreeMap<String, DataType>, pad: &str) -> Result<String, PlanError> {
        if self.fields.is_empty() {
            return Ok(format!("{pad}        out.push({});\n", self.value(&[])));
        }
        let mut out = format!("{pad}        {{\n");
        for (i, field) in self.fields.iter().enumerate() {
            out.push_str(&format!(
                "{pad}            let s{i} = {};\n",
                samples_of(field, types)?
            ));
        }
        for i in 0..self.fields.len() {
            out.push_str(&format!(
                "{pad}            for &v in &s{i} {{\n{pad}                out.push({});\n{pad}            }}\n",
                self.value(&self.varying(i))
            ));
        }
        out.push_str(&format!(
            "{pad}            out.push({});\n{pad}        }}\n",
            self.value(&self.every_field_last())
        ));
        Ok(out)
    }

    /// The arguments with field `varied` at the loop's value `v` and every
    /// other field at its first sample value.
    fn varying(&self, varied: usize) -> Vec<String> {
        (0..self.fields.len())
            .map(|j| {
                if j == varied {
                    "v".to_string()
                } else {
                    format!("{}(&s{j})", helper("first"))
                }
            })
            .collect()
    }

    /// The arguments with every field at its last sample value.
    fn every_field_last(&self) -> Vec<String> {
        (0..self.fields.len())
            .map(|j| format!("{}(&s{j})", helper("last")))
            .collect()
    }
}
