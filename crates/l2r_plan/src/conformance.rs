//! The derived conformance harness of a `v2` model (`l2r.conformance/v1`).
//!
//! Two programs, derived from the same validated model and never part of a
//! generation plan:
//!
//! - a Rust program that drives the generated `System` through operation
//!   sequences (every sequence up to a length, then pseudo-random longer ones)
//!   and records what it observes after every operation;
//! - a Lean program that replays each sequence on the Lean model of the
//!   scheduler (`L2R.Async1.Ports` over the derived network) and requires
//!   identical observations.
//!
//! An operation is a send of one sample message to an input port, a
//! `step_once`, or a take from an output port. An observation is the
//! operation's result and the whole system: every machine's state and mailbox
//! and every outbox, rendered as Rust's `Debug` prints them; the Lean side has
//! a generated renderer per data type that prints the same text.
//!
//! This is **testing**. It ties the generated Rust to the Lean scheduler model
//! on the sequences tried, for the sample messages chosen, and no further.

use std::collections::BTreeMap;

use l2r_model::digest;
use l2r_validate::Validated;
use l2r_validate::data::{DataType, FieldType, Shape};
use l2r_validate::flat::Flat;

use crate::PlanError;
use crate::lean_composition::{SEMANTICS_VERSION_V1, semantics_digest_v1};
use crate::lean_composition_v2::{PORTS_SCHEDULER_VERSION, ports_scheduler_digest};
use crate::plan::GeneratorIdentity;

/// Format identifier of both derived programs.
pub const CONFORMANCE_FORMAT: &str = "l2r.conformance/v1";

/// Which of the two programs to derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceSide {
    /// The recorder, driving the generated Rust.
    Rust,
    /// The replayer, on the Lean scheduler model.
    Lean,
}

/// Most sample messages offered to one input port.
const MESSAGES_PER_PORT: usize = 6;

/// A concrete value of a data type.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
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

type Types = BTreeMap<String, DataType>;

fn missing(name: &str) -> PlanError {
    PlanError(format!("type `{name}` was not resolved"))
}

/// Sample values of a field type: for an integer the smallest values, the
/// middle and the largest; for data, every constructor, each field taking each
/// of its sample values once while the others hold their first.
fn samples(ty: &FieldType, types: &Types) -> Result<Vec<Value>, PlanError> {
    let ints = |bits: u32| {
        let max = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        vec![0, 1, max / 2 + 1, max]
            .into_iter()
            .map(Value::UInt)
            .collect::<Vec<_>>()
    };
    Ok(match ty {
        FieldType::Bool => vec![Value::Bool(false), Value::Bool(true)],
        FieldType::U8 => ints(8),
        FieldType::U16 => ints(16),
        FieldType::U32 => ints(32),
        FieldType::U64 => ints(64),
        FieldType::Option(inner) => {
            let mut out = vec![Value::None];
            out.extend(
                samples(inner, types)?
                    .into_iter()
                    .map(|v| Value::Some(Box::new(v))),
            );
            out
        }
        FieldType::Named(name) => data_samples(name, types)?,
    })
}

fn data_samples(name: &str, types: &Types) -> Result<Vec<Value>, PlanError> {
    let data = types.get(name).ok_or_else(|| missing(name))?;
    let constructors: Vec<Vec<&FieldType>> = match &data.shape {
        Shape::Enum(variants) => variants.iter().map(|v| v.fields.iter().collect()).collect(),
        Shape::Struct { fields, .. } => vec![fields.iter().map(|f| &f.ty).collect()],
    };
    // Per constructor, then interleaved, so that a cap keeps every constructor.
    let mut per_constructor: Vec<Vec<Value>> = Vec::new();
    for (index, fields) in constructors.iter().enumerate() {
        let lists = fields
            .iter()
            .map(|f| samples(f, types))
            .collect::<Result<Vec<_>, _>>()?;
        let make = |args: Vec<Value>| Value::Data {
            ty: name.to_string(),
            index,
            args,
        };
        let base: Vec<Value> = lists.iter().map(|l| l[0].clone()).collect();
        let mut values = vec![make(base.clone())];
        for (i, list) in lists.iter().enumerate() {
            for v in list {
                let mut args = base.clone();
                args[i] = v.clone();
                values.push(make(args));
            }
        }
        values.push(make(
            lists
                .iter()
                .map(|l| l[l.len() - 1].clone())
                .collect::<Vec<_>>(),
        ));
        let mut unique: Vec<Value> = Vec::new();
        for v in values {
            if !unique.contains(&v) {
                unique.push(v);
            }
        }
        per_constructor.push(unique);
    }
    let longest = per_constructor.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..longest {
        for values in &per_constructor {
            if let Some(v) = values.get(i) {
                out.push(v.clone());
            }
        }
    }
    Ok(out)
}

/// `crate_name::types::tank::Level`.
fn rust_path(data: &DataType, krate: &str) -> String {
    data.rust_path().replacen("crate", krate, 1)
}

fn rust_expr(value: &Value, types: &Types, krate: &str) -> Result<String, PlanError> {
    Ok(match value {
        Value::Bool(b) => b.to_string(),
        Value::UInt(n) => n.to_string(),
        Value::None => "None".into(),
        Value::Some(inner) => format!("Some({})", rust_expr(inner, types, krate)?),
        Value::Data { ty, index, args } => {
            let data = types.get(ty).ok_or_else(|| missing(ty))?;
            let path = rust_path(data, krate);
            let args = args
                .iter()
                .map(|a| rust_expr(a, types, krate))
                .collect::<Result<Vec<_>, _>>()?;
            match &data.shape {
                Shape::Struct { fields, .. } if fields.is_empty() => format!("{path} {{}}"),
                Shape::Struct { fields, .. } => format!(
                    "{path} {{ {} }}",
                    fields
                        .iter()
                        .zip(&args)
                        .map(|(f, a)| format!("{}: {a}", f.rust_name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Shape::Enum(variants) if args.is_empty() => {
                    format!("{path}::{}", variants[*index].rust_name)
                }
                Shape::Enum(variants) => {
                    format!(
                        "{path}::{}({})",
                        variants[*index].rust_name,
                        args.join(", ")
                    )
                }
            }
        }
    })
}

fn lean_expr(value: &Value, types: &Types) -> Result<String, PlanError> {
    Ok(match value {
        Value::Bool(b) => b.to_string(),
        Value::UInt(n) => n.to_string(),
        Value::None => "none".into(),
        Value::Some(inner) => format!("(some {})", lean_expr(inner, types)?),
        Value::Data { ty, index, args } => {
            let data = types.get(ty).ok_or_else(|| missing(ty))?;
            let constructor = match &data.shape {
                Shape::Struct { constructor, .. } => constructor.clone(),
                Shape::Enum(variants) => variants[*index].lean_name.clone(),
            };
            let mut text = format!("(_root_.{constructor}");
            for arg in args {
                text.push(' ');
                text.push_str(&lean_expr(arg, types)?);
            }
            text.push_str(&format!(" : _root_.{ty})"));
            text
        }
    })
}

/// One operation of the alphabet.
enum Op {
    Send { port: usize, message: Value },
    Step,
    Take { sink: usize },
}

struct Harness<'a> {
    validated: &'a Validated,
    flat: &'a Flat,
    types: &'a Types,
    ops: Vec<Op>,
}

impl<'a> Harness<'a> {
    fn new(validated: &'a Validated) -> Result<Self, PlanError> {
        let flat = validated
            .flat()
            .ok_or_else(|| PlanError("conformance is derived for v2 models only".into()))?;
        let types = validated.types();
        let mut ops = Vec::new();
        for (port, input) in flat.inputs.iter().enumerate() {
            for message in data_samples(&input.message, types)?
                .into_iter()
                .take(MESSAGES_PER_PORT)
            {
                ops.push(Op::Send { port, message });
            }
        }
        ops.push(Op::Step);
        for sink in 0..flat.sinks.len() {
            ops.push(Op::Take { sink });
        }
        Ok(Self {
            validated,
            flat,
            types,
            ops,
        })
    }

    fn header(&self, comment: &str) -> String {
        let model = self.validated.model();
        let generator = GeneratorIdentity::current();
        format!(
            "{comment} @generated by L2R ({CONFORMANCE_FORMAT}). Do not edit: change the architecture model or the\n\
             {comment} Lean specification, then run `l2r conformance` again.\n\
             {comment} generator: {} {}\n\
             {comment} semantics: {SEMANTICS_VERSION_V1} {}\n\
             {comment} scheduler: {PORTS_SCHEDULER_VERSION} {}\n\
             {comment} model: {}\n\
             {comment} inventory: {}\n",
            generator.name,
            generator.version,
            semantics_digest_v1(),
            ports_scheduler_digest(),
            digest::digest_of(model),
            digest::digest_of(self.validated.inventory()),
        )
    }

    fn op_doc(&self, comment: &str) -> Result<String, PlanError> {
        let mut out = String::new();
        for (i, op) in self.ops.iter().enumerate() {
            let what = match op {
                Op::Send { port, message } => format!(
                    "send to `{}`: {}",
                    self.flat.inputs[*port].port,
                    lean_expr(message, self.types)?.replace("_root_.", "")
                ),
                Op::Step => "step_once".into(),
                Op::Take { sink } => format!("take from `{}`", self.flat.sinks[*sink].port),
            };
            out.push_str(&format!("{comment}   {i}: {what}\n"));
        }
        Ok(out)
    }

    fn rust(&self) -> Result<String, PlanError> {
        let model = self.validated.model();
        let krate = &model.system.rust_crate;
        let root = model
            .components
            .iter()
            .find(|c| c.parent.is_none())
            .ok_or_else(|| PlanError("no root component".into()))?;
        let mut out = self.header("//");
        out.push_str(&format!(
            "\n//! Records what the generated `System` of `{}` does on operation sequences,\n\
             //! for comparison with the Lean scheduler model by the derived `Conformance.lean`.\n\
             //!\n\
             //! This is bounded testing, not proof.\n\
             //!\n\
             //! Usage: `<program> <exhaustive length> <random sequences> <random length>`.\n\
             //! Output: one line per sequence, tab-separated: the operations (indices\n\
             //! joined by `.`), then one observation per operation: the result, every\n\
             //! machine's state and mailbox, and every outbox, as `Debug` prints them.\n\
             //!\n\
             //! Operations:\n//!\n",
            model.system.id
        ));
        out.push_str(&self.op_doc("//!")?);
        out.push_str(&format!(
            "\nuse {krate}::{}::{{Progress, System}};\n\nconst OPS: usize = {};\n\n\
             /// The index of `step_once`. A pseudo-random sequence takes it every second\n\
             /// operation on average, so that messages are delivered and not only offered.\n\
             const STEP: usize = {};\n\n",
            root.rust_module,
            self.ops.len(),
            self.ops
                .iter()
                .position(|op| matches!(op, Op::Step))
                .unwrap_or_default()
        ));
        out.push_str(
            "fn list<T: std::fmt::Debug>(len: usize, get: impl Fn(usize) -> Option<T>) -> String {\n    \
                 let items: Vec<String> = (0..len)\n        \
                     .map(|i| format!(\"{:?}\", get(i).expect(\"within length\")))\n        \
                     .collect();\n    \
                 format!(\"[{}]\", items.join(\";\"))\n}\n\n\
             fn observe(system: &System) -> String {\n    let mut parts: Vec<String> = Vec::new();\n",
        );
        for leaf in &self.flat.leaves {
            out.push_str(&format!(
                "    parts.push(format!(\"{{:?}}\", system.{id}_state()));\n    \
                     parts.push(list(system.{id}_inbox().len(), |i| system.{id}_inbox().get(i)));\n",
                id = leaf.id
            ));
        }
        for sink in &self.flat.sinks {
            out.push_str(&format!(
                "    parts.push(list(system.{id}_outbox().len(), |i| system.{id}_outbox().get(i)));\n",
                id = sink.port
            ));
        }
        out.push_str("    parts.join(\"|\")\n}\n\nfn apply(system: &mut System, op: usize) -> String {\n    let result = match op {\n");
        for (i, op) in self.ops.iter().enumerate() {
            match op {
                Op::Send { port, message } => out.push_str(&format!(
                    "        {i} => match system.send_{}({}) {{\n            Ok(()) => \"ok\".to_string(),\n            Err(_) => \"rej\".to_string(),\n        }},\n",
                    self.flat.inputs[*port].port,
                    rust_expr(message, self.types, krate)?
                )),
                Op::Step => out.push_str(&format!(
                    "        {i} => match system.step_once() {{\n            Progress::Delivered => \"D\".to_string(),\n            Progress::Settled => \"S\".to_string(),\n            Progress::Blocked => \"B\".to_string(),\n        }},\n"
                )),
                Op::Take { sink } => out.push_str(&format!(
                    "        {i} => format!(\"T:{{:?}}\", system.take_{}()),\n",
                    self.flat.sinks[*sink].port
                )),
            }
        }
        out.push_str(
            "        _ => \"?\".to_string(),\n    };\n    format!(\"{result}|{}\", observe(system))\n}\n\n\
             fn record(ops: &[usize]) {\n    \
                 let mut system = System::new();\n    \
                 let names: Vec<String> = ops.iter().map(|op| op.to_string()).collect();\n    \
                 let observed: Vec<String> = ops.iter().map(|&op| apply(&mut system, op)).collect();\n    \
                 println!(\"{}\\t{}\", names.join(\".\"), observed.join(\"\\t\"));\n}\n\n\
             fn main() {\n    \
                 let args: Vec<usize> = std::env::args()\n        \
                     .skip(1)\n        \
                     .map(|a| a.parse().expect(\"a number\"))\n        \
                     .collect();\n    \
                 let [exhaustive, random_count, random_length] = args[..] else {\n        \
                     panic!(\"usage: <exhaustive length> <random sequences> <random length>\");\n    \
                 };\n    \
                 // Every sequence of exactly `exhaustive` operations.\n    \
                 for n in 0..OPS.pow(exhaustive as u32) {\n        \
                     let mut code = n;\n        \
                     let ops: Vec<usize> = (0..exhaustive)\n            \
                         .map(|_| {\n                \
                             let op = code % OPS;\n                \
                             code /= OPS;\n                \
                             op\n            \
                         })\n            \
                         .collect();\n        \
                     record(&ops);\n    \
                 }\n    \
                 // Longer sequences from a fixed linear congruential generator.\n    \
                 let mut state: u64 = 0x9e37_79b9_7f4a_7c15;\n    \
                 for _ in 0..random_count {\n        \
                     let ops: Vec<usize> = (0..random_length)\n            \
                         .map(|_| {\n                \
                             state = state\n                    \
                                 .wrapping_mul(6_364_136_223_846_793_005)\n                    \
                                 .wrapping_add(1_442_695_040_888_963_407);\n                \
                             let draw = (state >> 33) as usize;\n                \
                             if draw.is_multiple_of(2) { STEP } else { (draw / 2) % OPS }\n            \
                         })\n            \
                         .collect();\n        \
                     record(&ops);\n    \
                 }\n}\n",
        );
        Ok(out)
    }

    /// Data types in dependency order, each with the name of its renderer.
    fn renderers(&self) -> Vec<(&'a DataType, String)> {
        let mut order: Vec<&DataType> = Vec::new();
        fn visit<'a>(data: &'a DataType, types: &'a Types, order: &mut Vec<&'a DataType>) {
            if order.iter().any(|d| d.lean_name == data.lean_name) {
                return;
            }
            for dependency in data.dependencies() {
                if let Some(d) = types.get(dependency) {
                    visit(d, types, order);
                }
            }
            order.push(data);
        }
        for data in self.types.values() {
            visit(data, self.types, &mut order);
        }
        order
            .into_iter()
            .enumerate()
            .map(|(i, d)| (d, format!("show{i}")))
            .collect()
    }

    fn lean(&self) -> Result<String, PlanError> {
        let model = self.validated.model();
        let root = model
            .components
            .iter()
            .find(|c| c.parent.is_none())
            .ok_or_else(|| PlanError("no root component".into()))?;
        let renderers = self.renderers();
        let renderer = |name: &str| {
            renderers
                .iter()
                .find(|(d, _)| d.lean_name == name)
                .map(|(_, f)| f.clone())
                .ok_or_else(|| missing(name))
        };
        fn show(
            ty: &FieldType,
            renderer: &dyn Fn(&str) -> Result<String, PlanError>,
        ) -> Result<String, PlanError> {
            Ok(match ty {
                FieldType::Named(name) => renderer(name)?,
                FieldType::Option(inner) => format!("(showOption {})", show(inner, renderer)?),
                _ => "toString".into(),
            })
        }
        let mut out = self.header("--");
        out.push_str(&format!(
            "import L2RDerived.{id}\n\n/-!\n# Bounded conformance of the generated scheduler of `{id}` with its Lean model\n\n\
             The derived Rust program records what the generated `System` does on operation\n\
             sequences. This program replays each sequence on `L2R.Async1.Ports` over the\n\
             derived network and requires identical observations: each operation's result,\n\
             and after it every machine's state and mailbox and every outbox.\n\n\
             This is **testing**, bounded by the sequences and sample messages tried. Run with\n\
             `lean --run Conformance.lean <traces>`.\n\nOperations:\n\n",
            id = model.system.id
        ));
        out.push_str(&self.op_doc("")?);
        out.push_str(&format!(
            "-/\n\nnamespace L2RConformance.«{id}»\n\nopen L2R.Async1\n\n\
             abbrev N : Network := L2RDerived.«{id}».«{root}».network\n\
             abbrev machines : Nat := L2RDerived.«{id}».«{root}».machines\n\
             abbrev Sched := Ports.Sched N\n\n\
             def showOption {{α : Type}} (f : α → String) : Option α → String\n  \
               | none => \"None\"\n  | some x => \"Some(\" ++ f x ++ \")\"\n\n\
             def showList {{α : Type}} (f : α → String) (xs : List α) : String :=\n  \
               \"[\" ++ \";\".intercalate (xs.map f) ++ \"]\"\n\n",
            id = model.system.id,
            root = root.id
        ));
        // One renderer per data type, printing what Rust's `Debug` prints.
        for (data, name) in &renderers {
            out.push_str(&format!(
                "def {name} : _root_.{} → String\n",
                data.lean_name
            ));
            match &data.shape {
                Shape::Struct { fields, .. } => {
                    let binders: Vec<String> = (0..fields.len()).map(|i| format!("a{i}")).collect();
                    let body = if fields.is_empty() {
                        format!("\"{}\"", data.rust_name)
                    } else {
                        let parts = fields
                            .iter()
                            .enumerate()
                            .map(|(i, f)| {
                                Ok(format!(
                                    "\"{}: \" ++ {} a{i}",
                                    f.rust_name,
                                    show(&f.ty, &renderer)?
                                ))
                            })
                            .collect::<Result<Vec<_>, PlanError>>()?;
                        format!(
                            "\"{} {{ \" ++ {} ++ \" }}\"",
                            data.rust_name,
                            parts.join(" ++ \", \" ++ ")
                        )
                    };
                    out.push_str(&format!("  | ⟨{}⟩ => {body}\n\n", binders.join(", ")));
                }
                Shape::Enum(variants) => {
                    for variant in variants {
                        let binders: Vec<String> = (0..variant.fields.len())
                            .map(|i| format!(" a{i}"))
                            .collect();
                        let body = if variant.fields.is_empty() {
                            format!("\"{}\"", variant.rust_name)
                        } else {
                            let parts = variant
                                .fields
                                .iter()
                                .enumerate()
                                .map(|(i, f)| Ok(format!("{} a{i}", show(f, &renderer)?)))
                                .collect::<Result<Vec<_>, PlanError>>()?;
                            format!(
                                "\"{}(\" ++ {} ++ \")\"",
                                variant.rust_name,
                                parts.join(" ++ \", \" ++ ")
                            )
                        };
                        out.push_str(&format!(
                            "  | _root_.{}{} => {body}\n",
                            variant.lean_name,
                            binders.concat()
                        ));
                    }
                    out.push('\n');
                }
            }
        }
        // Typed views of the network's children.
        let behavior = |id: &str| {
            model
                .component(id)
                .and_then(|c| c.behavior.clone())
                .ok_or_else(|| PlanError(format!("machine `{id}` has no behavior")))
        };
        let mut parts = Vec::new();
        for (i, leaf) in self.flat.leaves.iter().enumerate() {
            let b = behavior(&leaf.id)?;
            out.push_str(&format!(
                "def state{i} (k : Config N) : _root_.{} := k.state ⟨{i}, by decide⟩\n\
                 def inbox{i} (k : Config N) : List _root_.{} := k.inbox ⟨{i}, by decide⟩\n",
                b.owned_state, b.input
            ));
            parts.push(format!("{} (state{i} k)", renderer(&b.owned_state)?));
            parts.push(format!("showList {} (inbox{i} k)", renderer(&b.input)?));
        }
        let machines = self.flat.leaves.len();
        for (i, sink) in self.flat.sinks.iter().enumerate() {
            let index = machines + i;
            out.push_str(&format!(
                "def inbox{index} (k : Config N) : List _root_.{} := k.inbox ⟨{index}, by decide⟩\n",
                sink.message
            ));
            parts.push(format!(
                "showList {} (inbox{index} k)",
                renderer(&sink.message)?
            ));
        }
        out.push_str(&format!(
            "\ndef observe (k : Config N) : String :=\n  \"|\".intercalate [\n    {}]\n\n",
            parts.join(",\n    ")
        ));
        out.push_str("def applyOp (s : Sched) : Nat → Sched × String\n");
        let n = machines + self.flat.sinks.len();
        for (i, op) in self.ops.iter().enumerate() {
            match op {
                Op::Send { port, message } => out.push_str(&format!(
                    "  | {i} =>\n    match Ports.send N s (⟨{port}, by decide⟩ : Fin N.ports) {} with\n    | some s' => (s', \"ok\")\n    | none => (s, \"rej\")\n",
                    lean_expr(message, self.types)?
                )),
                Op::Step => out.push_str(&format!(
                    "  | {i} =>\n    match Ports.stepOnce N machines s with\n    | .delivered s' => (s', \"D\")\n    | .settled => (s, \"S\")\n    | .blocked => (s, \"B\")\n"
                )),
                Op::Take { sink } => {
                    let index = machines + sink;
                    out.push_str(&format!(
                        "  | {i} =>\n    match Ports.take N s (⟨{index}, by decide⟩ : Fin {n}) with\n    | some s' => (s', \"T:\" ++ showOption {} (inbox{index} s.cfg).head?)\n    | none => (s, \"T:None\")\n",
                        renderer(&self.flat.sinks[*sink].message)?
                    ));
                }
            }
        }
        out.push_str(
            "  | _ => (s, \"?\")\n\n\
             def replay (ops : List Nat) : String := Id.run do\n  \
               let mut s : Sched := Ports.start N\n  \
               let mut out : Array String := #[]\n  \
               for op in ops do\n    \
                 let (s', result) := applyOp s op\n    \
                 s := s'\n    \
                 out := out.push (result ++ \"|\" ++ observe s.cfg)\n  \
               return \"\\t\".intercalate out.toList\n\n",
        );
        out.push_str(&format!(
            "end L2RConformance.«{id}»\n\n\
             def main (args : List String) : IO UInt32 := do\n  \
               let some path := args.head? | IO.eprintln \"usage: Conformance <traces>\"; return 2\n  \
               let lines := (← IO.FS.lines path).filter (· ≠ \"\")\n  \
               let mut bad := 0\n  \
               for line in lines do\n    \
                 match line.splitOn \"\\t\" with\n    \
                 | ops :: observed =>\n      \
                   let expected := \"\\t\".intercalate observed\n      \
                   let actual := L2RConformance.«{id}».replay ((ops.splitOn \".\").filterMap String.toNat?)\n      \
                   if actual != expected then\n        \
                     bad := bad + 1\n        \
                     if bad ≤ 3 then\n          \
                       IO.println s!\"MISMATCH {{ops}}\\n  rust: {{expected}}\\n  lean: {{actual}}\"\n    \
                 | [] => bad := bad + 1\n  \
               IO.println s!\"CONFORMANCE {{lines.size - bad}}/{{lines.size}} sequences agree\"\n  \
               return if bad == 0 && lines.size > 0 then 0 else 1\n",
            id = model.system.id
        ));
        Ok(out)
    }
}

/// Derives one side of the conformance harness of a `v2` model.
pub fn conformance(validated: &Validated, side: ConformanceSide) -> Result<String, PlanError> {
    let harness = Harness::new(validated)?;
    match side {
        ConformanceSide::Rust => harness.rust(),
        ConformanceSide::Lean => harness.lean(),
    }
}
