import CrossingProperties

/-!
# Bounded conformance: the generated Rust scheduler vs. the Lean scheduler model

`scheduler_traces` (Rust) records what the generated `System` does on every
operation sequence up to a fixed length. This program replays each sequence on
`L2R.Async.RoundRobin` and requires identical observations: each
`step_once` result and both machines' states after every operation.

This is **testing**, bounded by the sequence length. It is evidence for the one
link that is reviewed rather than proved: that the Lean scheduler model, whose
runs are proved to be permitted behaviours, describes the generated Rust.
Run with `lean --run Conformance.lean <traces>`.
-/

open L2R.Async CrossingProperties Crossing

/-- Rust variant name of a Lean constructor: its last name component, capitalised
(the generator's naming rule for field-less inductives). -/
def variantName {α : Type} [Repr α] (x : α) : String :=
  ((reprStr x).splitOn ".").getLast!.capitalize

abbrev Sched := RoundRobin.Sched N

def applyOp (s : Sched) : Char → Sched × String
  | 'p' => (RoundRobin.send N s L Latch.Input.buttonPressed, "-")
  | 'a' => (RoundRobin.send N s L Latch.Input.acknowledged, "-")
  | 'r' => (RoundRobin.send N s G Signal.Input.crossingRequested, "-")
  | 't' => (RoundRobin.send N s G Signal.Input.timerExpired, "-")
  | _ =>
    match RoundRobin.stepOnce N s with
    | some s' => (s', "1")
    | none => (s, "0")

def replay (ops : String) : String := Id.run do
  let mut s : Sched := RoundRobin.start N
  let mut out : Array String := #[]
  for op in ops.toList do
    let (s', step) := applyOp s op
    s := s'
    out := out.push s!"{step}/{variantName (latch s.cfg)}/{variantName (signal s.cfg)}"
  return " ".intercalate out.toList

def main (args : List String) : IO UInt32 := do
  let some path := args.head? | IO.eprintln "usage: Conformance <traces>"; return 2
  let lines := (← IO.FS.lines path).filter (· ≠ "")
  let mut bad := 0
  for line in lines do
    match line.splitOn " " with
    | ops :: observed =>
      let expected := " ".intercalate observed
      let actual := replay ops
      if actual != expected then
        bad := bad + 1
        if bad ≤ 3 then
          IO.println s!"MISMATCH {ops}\n  rust: {expected}\n  lean: {actual}"
    | [] => bad := bad + 1
  IO.println s!"CONFORMANCE {lines.size - bad}/{lines.size} sequences agree"
  return if bad == 0 && lines.size > 0 then 0 else 1
