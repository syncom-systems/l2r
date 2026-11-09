import CrossingV1Properties

/-!
# Bounded conformance: the generated Rust scheduler vs. the Lean scheduler model

`scheduler_traces` (Rust) records what the generated `composite/v1` `System`
does on every operation sequence up to a fixed length. This program replays each
sequence on `L2R.Async1.RoundRobin` and requires identical observations: each
send's accept/reject, each `step_once` result (delivered, settled or blocked),
both machines' states and **both mailboxes' contents** after every operation.

This is **testing**, bounded by the sequence length. It is evidence for the link
between the Lean scheduler model, whose runs are proved to be permitted
behaviours, and the generated Rust, beyond what the Rust-to-Lean refinement
proofs of `verification/aeneas_v1` cover. Run with
`lean --run Conformance.lean <traces>`.
-/

open L2R.Async1 CrossingV1Properties CrossingV1

/-- Rust variant name of a Lean constructor: its last name component, capitalised
(the generator's naming rule for field-less inductives). -/
def variantName {α : Type} [Repr α] (x : α) : String :=
  ((reprStr x).splitOn ".").getLast!.capitalize

def mailbox {α : Type} [Repr α] (xs : List α) : String :=
  if xs.isEmpty then "-" else ",".intercalate (xs.map variantName)

abbrev Sched := RoundRobin.Sched N

def applyOp (s : Sched) : Char → Sched × String
  | 'p' =>
    match RoundRobin.send N s ⟨0, by decide⟩ (Ports.ButtonEvent.pressed : N.Msg ⟨0, by decide⟩) with
    | some s' => (s', "ok")
    | none => (s, "rej")
  | 't' =>
    match RoundRobin.send N s ⟨1, by decide⟩ (Ports.TimerEvent.expired : N.Msg ⟨1, by decide⟩) with
    | some s' => (s', "ok")
    | none => (s, "rej")
  | _ =>
    match RoundRobin.stepOnce N s with
    | .delivered s' => (s', "D")
    | .settled => (s, "S")
    | .blocked => (s, "B")

def replay (ops : String) : String := Id.run do
  let mut s : Sched := RoundRobin.start N
  let mut out : Array String := #[]
  for op in ops.toList do
    let (s', result) := applyOp s op
    s := s'
    out := out.push s!"{result}/{variantName (latch s.cfg)}/{variantName (signal s.cfg)}/{mailbox (latchIn s.cfg)}/{mailbox (signalIn s.cfg)}"
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
