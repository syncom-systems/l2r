//! The Lean side of the harness: a program that replays each recorded sequence
//! on `L2R.Async1.Ports` over the derived network and compares observations.
//!
//! [`replayer`] is the outline of the emitted file; every other item emits the
//! section it is named after.

use l2r_model::{Behavior, Component};

use super::renderers::Renderers;
use super::{Harness, Op};
use crate::PlanError;

/// The whole replayer program.
pub(super) fn replayer(harness: &Harness) -> Result<String, PlanError> {
    let root = harness.root()?;
    let renderers = Renderers::new(harness.types);
    Ok([
        harness.header("--"),
        module_doc(harness)?,
        namespace_opening(harness, root),
        renderers.definitions()?,
        observe(harness, &renderers)?,
        apply_op(harness, &renderers)?,
        REPLAY.to_string(),
        namespace_end(harness),
        main(harness),
    ]
    .concat())
}

fn module_doc(harness: &Harness) -> Result<String, PlanError> {
    Ok(format!(
        "import L2RDerived.{id}\n\n/-!\n# Bounded conformance of the generated scheduler of `{id}` with its Lean model\n\n\
         The derived Rust program records what the generated `System` does on operation\n\
         sequences. This program replays each sequence on `L2R.Async1.Ports` over the\n\
         derived network and requires identical observations: each operation's result,\n\
         and after it every machine's state and mailbox and every outbox.\n\n\
         This is **testing**, bounded by the sequences and sample messages tried. Run with\n\
         `lean --run Conformance.lean <traces>`.\n\nOperations:\n\n{operations}-/\n\n",
        id = harness.system_id(),
        operations = harness.op_doc("")?
    ))
}

/// The namespace, and the derived network and its scheduler under short names.
fn namespace_opening(harness: &Harness, root: &Component) -> String {
    format!(
        "namespace L2RConformance.«{id}»\n\nopen L2R.Async1\n\n\
         abbrev N : Network := L2RDerived.«{id}».«{root}».network\n\
         abbrev machines : Nat := L2RDerived.«{id}».«{root}».machines\n\
         abbrev Sched := Ports.Sched N\n\n",
        id = harness.system_id(),
        root = root.id
    )
}

/// A typed view of one child of the network's configuration, and how the
/// observation renders it.
struct View {
    definition: String,
    rendering: String,
}

/// The state of machine `child`, of the Lean type `ty`.
fn state_view(child: usize, ty: &str, renderers: &Renderers) -> Result<View, PlanError> {
    Ok(View {
        definition: format!(
            "def state{child} (k : Config N) : _root_.{ty} := k.state ⟨{child}, by decide⟩\n"
        ),
        rendering: format!("{} (state{child} k)", renderers.name(ty)?),
    })
}

/// The mailbox of `child` (a machine's inbox or an outbox), holding messages
/// of the Lean type `ty`.
fn inbox_view(child: usize, ty: &str, renderers: &Renderers) -> Result<View, PlanError> {
    Ok(View {
        definition: format!(
            "def inbox{child} (k : Config N) : List _root_.{ty} := k.inbox ⟨{child}, by decide⟩\n"
        ),
        rendering: format!("showList {} (inbox{child} k)", renderers.name(ty)?),
    })
}

fn behavior<'a>(harness: &Harness<'a>, machine: &str) -> Result<&'a Behavior, PlanError> {
    harness
        .validated
        .model()
        .component(machine)
        .and_then(|c| c.behavior.as_ref())
        .ok_or_else(|| PlanError(format!("machine `{machine}` has no behavior")))
}

/// The views in the order the recorder observes: every machine's state and
/// mailbox, then every outbox.
fn views(harness: &Harness, renderers: &Renderers) -> Result<Vec<View>, PlanError> {
    let mut views = Vec::new();
    for (i, leaf) in harness.flat.leaves.iter().enumerate() {
        let behavior = behavior(harness, &leaf.id)?;
        views.push(state_view(i, &behavior.owned_state, renderers)?);
        views.push(inbox_view(i, &behavior.input, renderers)?);
    }
    let machines = harness.flat.leaves.len();
    for (i, sink) in harness.flat.sinks.iter().enumerate() {
        views.push(inbox_view(machines + i, &sink.message, renderers)?);
    }
    Ok(views)
}

/// The typed views of the network's children, and `observe` over them.
fn observe(harness: &Harness, renderers: &Renderers) -> Result<String, PlanError> {
    let views = views(harness, renderers)?;
    let definitions: String = views.iter().map(|v| v.definition.as_str()).collect();
    let renderings: Vec<&str> = views.iter().map(|v| v.rendering.as_str()).collect();
    Ok(format!(
        "{definitions}\ndef observe (k : Config N) : String :=\n  \"|\".intercalate [\n    {}]\n\n",
        renderings.join(",\n    ")
    ))
}

/// `applyOp`: one operation, by index, on the scheduler model.
fn apply_op(harness: &Harness, renderers: &Renderers) -> Result<String, PlanError> {
    let mut out = String::from("def applyOp (s : Sched) : Nat → Sched × String\n");
    for (i, op) in harness.ops.iter().enumerate() {
        out.push_str(&apply_op_case(harness, renderers, i, op)?);
    }
    out.push_str("  | _ => (s, \"?\")\n\n");
    Ok(out)
}

/// The case of `applyOp` that performs operation `i`.
fn apply_op_case(
    harness: &Harness,
    renderers: &Renderers,
    i: usize,
    op: &Op,
) -> Result<String, PlanError> {
    Ok(match op {
        Op::Send { port, message } => format!(
            "  | {i} =>\n    match Ports.send N s (⟨{port}, by decide⟩ : Fin N.ports) {} with\n    | some s' => (s', \"ok\")\n    | none => (s, \"rej\")\n",
            message.lean_expr(harness.types)?
        ),
        Op::Step => format!(
            "  | {i} =>\n    match Ports.stepOnce N machines s with\n    | .delivered s' => (s', \"D\")\n    | .settled => (s, \"S\")\n    | .blocked => (s, \"B\")\n"
        ),
        Op::Take { sink } => {
            let machines = harness.flat.leaves.len();
            let index = machines + sink;
            let children = machines + harness.flat.sinks.len();
            format!(
                "  | {i} =>\n    match Ports.take N s (⟨{index}, by decide⟩ : Fin {children}) with\n    | some s' => (s', \"T:\" ++ showOption {} (inbox{index} s.cfg).head?)\n    | none => (s, \"T:None\")\n",
                renderers.name(&harness.flat.sinks[*sink].message)?
            )
        }
    })
}

/// `replay`: a sequence from the start, as the observations the recorder prints.
const REPLAY: &str = r#"def replay (ops : List Nat) : String := Id.run do
  let mut s : Sched := Ports.start N
  let mut out : Array String := #[]
  for op in ops do
    let (s', result) := applyOp s op
    s := s'
    out := out.push (result ++ "|" ++ observe s.cfg)
  return "\t".intercalate out.toList

"#;

fn namespace_end(harness: &Harness) -> String {
    format!("end L2RConformance.«{}»\n\n", harness.system_id())
}

/// `main`: replays every recorded line and reports how many agree.
fn main(harness: &Harness) -> String {
    format!(
        r#"def main (args : List String) : IO UInt32 := do
  let some path := args.head? | IO.eprintln "usage: Conformance <traces>"; return 2
  let lines := (← IO.FS.lines path).filter (· ≠ "")
  let mut bad := 0
  for line in lines do
    match line.splitOn "\t" with
    | ops :: observed =>
      let expected := "\t".intercalate observed
      let actual := L2RConformance.«{id}».replay ((ops.splitOn ".").filterMap String.toNat?)
      if actual != expected then
        bad := bad + 1
        if bad ≤ 3 then
          IO.println s!"MISMATCH {{ops}}\n  rust: {{expected}}\n  lean: {{actual}}"
    | [] => bad := bad + 1
  IO.println s!"CONFORMANCE {{lines.size - bad}}/{{lines.size}} sequences agree"
  return if bad == 0 && lines.size > 0 then 0 else 1
"#,
        id = harness.system_id()
    )
}
