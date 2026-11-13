//! The Rust side of the harness: a program that drives the generated `System`
//! through operation sequences and prints what it observes.
//!
//! [`recorder`] is the outline of the emitted file; every other item emits the
//! section it is named after.

use l2r_model::Component;
use l2r_model::rust_names::composite_v2;

use super::{Harness, Op};
use crate::PlanError;

/// The whole recorder program.
pub(super) fn recorder(harness: &Harness) -> Result<String, PlanError> {
    let root = harness.root()?;
    Ok([
        harness.header("//"),
        module_doc(harness)?,
        imports_and_constants(harness, root),
        LIST.to_string(),
        observe(harness),
        apply(harness)?,
        RECORD.to_string(),
        MAIN.to_string(),
    ]
    .concat())
}

fn module_doc(harness: &Harness) -> Result<String, PlanError> {
    Ok(format!(
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
         //! Operations:\n//!\n{}",
        harness.system_id(),
        harness.op_doc("//!")?
    ))
}

fn imports_and_constants(harness: &Harness, root: &Component) -> String {
    let krate = &harness.validated.model().system.rust_crate;
    let step = harness
        .ops
        .iter()
        .position(|op| matches!(op, Op::Step))
        .unwrap_or_default();
    format!(
        "\nuse {krate}::{}::{{Progress, System}};\n\nconst OPS: usize = {};\n\n\
         /// The index of `step_once`. A pseudo-random sequence takes it every second\n\
         /// operation on average, so that messages are delivered and not only offered.\n\
         const STEP: usize = {step};\n\n",
        root.rust_module,
        harness.ops.len(),
    )
}

/// `list`: a mailbox or an outbox as `Debug` prints its messages.
const LIST: &str = r#"fn list<T: std::fmt::Debug>(len: usize, get: impl Fn(usize) -> Option<T>) -> String {
    let items: Vec<String> = (0..len)
        .map(|i| format!("{:?}", get(i).expect("within length")))
        .collect();
    format!("[{}]", items.join(";"))
}

"#;

/// `observe`: every machine's state and mailbox, then every outbox.
fn observe(harness: &Harness) -> String {
    let mut out = String::from(
        "fn observe(system: &System) -> String {\n    let mut parts: Vec<String> = Vec::new();\n",
    );
    for leaf in &harness.flat.leaves {
        out.push_str(&format!(
            "    parts.push(format!(\"{{:?}}\", system.{state}()));\n    \
                 parts.push(list(system.{inbox}().len(), |i| system.{inbox}().get(i)));\n",
            state = composite_v2::Machine(&leaf.id).state(),
            inbox = composite_v2::Machine(&leaf.id).inbox()
        ));
    }
    for sink in &harness.flat.sinks {
        out.push_str(&format!(
            "    parts.push(list(system.{outbox}().len(), |i| system.{outbox}().get(i)));\n",
            outbox = composite_v2::OutPort(&sink.port).outbox()
        ));
    }
    out.push_str("    parts.join(\"|\")\n}\n\n");
    out
}

/// `apply`: one operation, by index, and what is observed after it.
fn apply(harness: &Harness) -> Result<String, PlanError> {
    let mut out = String::from(
        "fn apply(system: &mut System, op: usize) -> String {\n    let result = match op {\n",
    );
    for (i, op) in harness.ops.iter().enumerate() {
        out.push_str(&apply_arm(harness, i, op)?);
    }
    out.push_str(
        "        _ => \"?\".to_string(),\n    };\n    format!(\"{result}|{}\", observe(system))\n}\n\n",
    );
    Ok(out)
}

/// The arm of `apply` that performs operation `i`.
fn apply_arm(harness: &Harness, i: usize, op: &Op) -> Result<String, PlanError> {
    Ok(match op {
        Op::Send { port, message } => format!(
            "        {i} => match system.{}({}) {{\n            Ok(()) => \"ok\".to_string(),\n            Err(_) => \"rej\".to_string(),\n        }},\n",
            composite_v2::InPort(&harness.flat.inputs[*port].port).send(),
            message.rust_expr(harness.types, &harness.validated.model().system.rust_crate)?
        ),
        Op::Step => format!(
            "        {i} => match system.step_once() {{\n            Progress::Delivered => \"D\".to_string(),\n            Progress::Settled => \"S\".to_string(),\n            Progress::Blocked => \"B\".to_string(),\n        }},\n"
        ),
        Op::Take { sink } => format!(
            "        {i} => format!(\"T:{{:?}}\", system.{}()),\n",
            composite_v2::OutPort(&harness.flat.sinks[*sink].port).take()
        ),
    })
}

/// `record`: one sequence on a fresh system, as one output line.
const RECORD: &str = r#"fn record(ops: &[usize]) {
    let mut system = System::new();
    let names: Vec<String> = ops.iter().map(|op| op.to_string()).collect();
    let observed: Vec<String> = ops.iter().map(|&op| apply(&mut system, op)).collect();
    println!("{}\t{}", names.join("."), observed.join("\t"));
}

"#;

/// `main`: the exhaustive driver, then the pseudo-random one.
const MAIN: &str = r#"fn main() {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .map(|a| a.parse().expect("a number"))
        .collect();
    let [exhaustive, random_count, random_length] = args[..] else {
        panic!("usage: <exhaustive length> <random sequences> <random length>");
    };
    // Every sequence of exactly `exhaustive` operations.
    for n in 0..OPS.pow(exhaustive as u32) {
        let mut code = n;
        let ops: Vec<usize> = (0..exhaustive)
            .map(|_| {
                let op = code % OPS;
                code /= OPS;
                op
            })
            .collect();
        record(&ops);
    }
    // Longer sequences from a fixed linear congruential generator.
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..random_count {
        let ops: Vec<usize> = (0..random_length)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let draw = (state >> 33) as usize;
                if draw.is_multiple_of(2) { STEP } else { (draw / 2) % OPS }
            })
            .collect();
        record(&ops);
    }
}
"#;
