import Crossing
import CrossingV1

/-!
# Fair pending cycles, by exhaustive finite-state search (TESTED, NOT PROVED)

For a pair of machines wired as the crossing is (a latch emitting a request, a
signal emitting an acknowledgement, two typed ports, bounded mailboxes), this
program explores every reachable configuration and searches for a *fair pending
cycle*: a closed run in which the latch stays pending forever, timer expiries
keep being sent, and no delivery that is enabled throughout is starved. If one
exists, a pending request can wait forever **even under fairness**; if none
exists, the request is acknowledged on every such run.

It is run on the original machines (`Crossing`, the stranded-request defect)
and on the repaired ones (`CrossingV1`), for several capacities. The result is
evidence, not a proof: the search is exact for these finite systems, but it is
executed code, not a theorem about all runs. The proved statements are
`no_stranding` and the safety theorems in `CrossingV1Properties`.

The criterion is exact for a finite graph: a run that stays in the pending
region forever eventually stays in one strongly connected component and visits
a closed walk there; any set of edges and states of one component can be
visited by a single closed walk. So a fair pending cycle exists iff some
component has a timer-send edge and, for each delivery, either a state where
it is disabled or an edge that takes it.
-/

structure Spec where
  L : Type
  S : Type
  LI : Type
  SI : Type
  deqL : DecidableEq L
  deqS : DecidableEq S
  deqLI : DecidableEq LI
  deqSI : DecidableEq SI
  lstep : L → LI → L × Bool      -- the latch's new state, and whether it emits a request
  sstep : S → SI → S × Bool      -- the signal's new state, and whether it emits an acknowledgement
  l0 : L
  s0 : S
  pending : L → Bool
  press : LI
  ack : LI
  req : SI
  tick : SI

/-- A configuration: latch, signal, latch mailbox, signal mailbox. -/
abbrev Node (sp : Spec) := sp.L × sp.S × List sp.LI × List sp.SI

/-- Edge labels: 0 delivers to the latch, 1 to the signal, 2 sends a press, 3 sends a timer expiry. -/
def succs (sp : Spec) (lcap scap : Nat) (n : Node sp) : List (Nat × Node sp) :=
  letI := sp.deqLI; letI := sp.deqSI
  let (ls, ss, li, si) := n
  let dl : List (Nat × Node sp) :=
    match li with
    | i :: rest =>
      let (ls', emit) := sp.lstep ls i
      let out := if emit then [sp.req] else []
      if si.length + out.length ≤ scap then [(0, (ls', ss, rest, si ++ out))] else []
    | [] => []
  let dg : List (Nat × Node sp) :=
    match si with
    | i :: rest =>
      let (ss', emit) := sp.sstep ss i
      let out := if emit then [sp.ack] else []
      if li.length + out.length ≤ lcap then [(1, (ls, ss', li ++ out, rest))] else []
    | [] => []
  let pr : List (Nat × Node sp) := if li.length < lcap then [(2, (ls, ss, li ++ [sp.press], si))] else []
  let tk : List (Nat × Node sp) := if si.length < scap then [(3, (ls, ss, li, si ++ [sp.tick]))] else []
  dl ++ dg ++ pr ++ tk

partial def explore (sp : Spec) (lcap scap : Nat) : List (Node sp) :=
  letI := sp.deqL; letI := sp.deqS; letI := sp.deqLI; letI := sp.deqSI
  let rec go (seen : List (Node sp)) (todo : List (Node sp)) : List (Node sp) :=
    match todo with
    | [] => seen
    | n :: rest =>
      let new := ((succs sp lcap scap n).map (·.2)).filter fun m => !(seen.contains m) && !(rest.contains m)
      go (seen ++ [n]) (rest ++ new.eraseDups)
  go [] [(sp.l0, sp.s0, [], [])]

/-- Number of strongly connected components of the pending region that contain a fair pending cycle. -/
partial def fairPendingCycles (sp : Spec) (lcap scap : Nat) : Nat :=
  letI := sp.deqL; letI := sp.deqS; letI := sp.deqLI; letI := sp.deqSI
  let nodes := (explore sp lcap scap).filter fun n => sp.pending n.1
  let inP := fun (m : Node sp) => nodes.contains m
  let edges := fun (n : Node sp) => (succs sp lcap scap n).filter fun e => inP e.2
  let rec reach (frontier seen : List (Node sp)) : List (Node sp) :=
    match frontier with
    | [] => seen
    | n :: rest =>
      let new := ((edges n).map (·.2)).filter fun m => !(seen.contains m) && !(rest.contains m)
      reach (rest ++ new.eraseDups) (seen ++ [n])
  let reachSet := fun (n : Node sp) => reach [n] []
  let comps : List (List (Node sp)) :=
    nodes.foldl (fun acc n =>
      if acc.any (·.contains n) then acc
      else acc ++ [(reachSet n).filter fun m => (reachSet m).contains n]) []
  comps.foldl (fun count comp =>
    let inner : List (Nat × Node sp) := comp.flatMap fun n => (edges n).filter fun e => comp.contains e.2
    let labels : List Nat := inner.map (·.1)
    let enabled := fun (c : Nat) (n : Node sp) => (succs sp lcap scap n).any (·.1 == c)
    let wf := [0, 1].all fun c => comp.any (fun n => !enabled c n) || labels.contains c
    if labels.contains 3 && wf then count + 1 else count) 0

def original : Spec where
  L := Crossing.Latch.State
  S := Crossing.Signal.State
  LI := Crossing.Latch.Input
  SI := Crossing.Signal.Input
  deqL := inferInstance
  deqS := inferInstance
  deqLI := inferInstance
  deqSI := inferInstance
  lstep := fun s i => let r := Crossing.Latch.step s i; (r.1, r.2.isSome)
  sstep := fun s i => let r := Crossing.Signal.step s i; (r.1, r.2.isSome)
  l0 := Crossing.Latch.init
  s0 := Crossing.Signal.init
  pending := fun s => s == .pending
  press := .buttonPressed
  ack := .acknowledged
  req := .crossingRequested
  tick := .timerExpired

def repaired : Spec where
  L := CrossingV1.Latch.State
  S := CrossingV1.Signal.State
  LI := CrossingV1.Latch.Input
  SI := CrossingV1.Signal.Input
  deqL := inferInstance
  deqS := inferInstance
  deqLI := inferInstance
  deqSI := inferInstance
  lstep := fun s i => let r := CrossingV1.Latch.step s i; (r.1, r.2.isSome)
  sstep := fun s i => let r := CrossingV1.Signal.step s i; (r.1, r.2.isSome)
  l0 := CrossingV1.Latch.init
  s0 := CrossingV1.Signal.init
  pending := fun s => s == .pending
  press := .buttonPressed
  ack := .acknowledged
  req := .crossingRequested
  tick := .timerExpired

def main : IO UInt32 := do
  let mut bad := false
  for (lcap, scap) in [(1, 1), (1, 2), (2, 2)] do
    let o := fairPendingCycles original lcap scap
    let r := fairPendingCycles repaired lcap scap
    IO.println s!"FAIR-CYCLES capacities=({lcap},{scap}) original={o} repaired={r}"
    if o == 0 || r != 0 then bad := true
  if bad then
    IO.println "FAIR-CYCLES UNEXPECTED: the original must have a fair pending cycle and the repaired design none"
    return 1
  IO.println "FAIR-CYCLES as expected (tested, not proved)"
  return 0
