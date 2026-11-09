import L2R.Async1
import L2RDerived.crossing_v1

/-!
# The scheduler model, on the data of the crossing, and what is *proved* about it

`L2R.Async1.RoundRobin` is the Lean model of the `composite/v1` scheduler. It is stated over a
dependently typed `Network`, which is awkward to relate to straight-line Rust. This file bridges
the two without any trusted step:

* `V` is the scheduler's data with concrete types (two machine states, two mailbox contents, the
  cursor); `toSched : V → RoundRobin.Sched N` embeds it in the model, `ofSched` reads it back, and
  `toSched_ofSched` shows nothing is lost.
* `simOut`, `simSend0`, `simSend1` are plain functions on `V` that mirror the scheduler step by step
  (`att0`, `att1`, `attempt`).
* **`leanOut_eq`, `leanSend0_eq`, `leanSend1_eq`**: for *every* well-formed `V` (mailboxes within
  capacity, cursor in range), `RoundRobin.stepOnce` / `RoundRobin.send` of the derived network equal
  `simOut` / `simSend*`. Proved by kernel `decide` over the complete finite set of well-formed `V`
  (1,176 states), plus a proved coverage lemma (`mem_allV`). No `native_decide`.

`Refinement.lean` then proves that the extracted Rust refines `simOut` / `simSend*`.

Scope: the enumeration is specific to this network (2 children, capacity 2, 2 inputs each). A network
with other capacities needs a new `V`/enumeration; it does not need new Rust proofs by hand if the
generated shape is the same.
-/
open L2R.Async1
open L2RDerived.«crossing_v1».«crossing» (network)

namespace AeneasV1

abbrev N := network
abbrev LS := CrossingV1.Latch.State
abbrev SS := CrossingV1.Signal.State
abbrev LI := CrossingV1.Latch.Input
abbrev SI := CrossingV1.Signal.Input

/-- The scheduler's data, with concrete types. -/
structure V where
  a : LS
  b : SS
  l0 : List LI
  l1 : List SI
  next : Nat
  deriving DecidableEq

def mkCfg (v : V) : Config N where
  state := fun i => match i with
    | ⟨0, _⟩ => v.a
    | ⟨1, _⟩ => v.b
    | ⟨n + 2, h⟩ => absurd h (by change ¬ n + 2 < 2; omega)
  inbox := fun i => match i with
    | ⟨0, _⟩ => v.l0
    | ⟨1, _⟩ => v.l1
    | ⟨n + 2, h⟩ => absurd h (by change ¬ n + 2 < 2; omega)

def toSched (v : V) : RoundRobin.Sched N := ⟨mkCfg v, v.next⟩

def ofSched (s : RoundRobin.Sched N) : V :=
  ⟨s.cfg.state ⟨0, by decide⟩, s.cfg.state ⟨1, by decide⟩, s.cfg.inbox ⟨0, by decide⟩, s.cfg.inbox ⟨1, by decide⟩, s.next⟩

inductive Out where
  | delivered (v : V)
  | settled
  | blocked
  deriving DecidableEq

def leanOut (v : V) : Out :=
  match RoundRobin.stepOnce N (toSched v) with
  | .delivered s' => .delivered (ofSched s')
  | .settled => .settled
  | .blocked => .blocked

inductive Att | delivered | blocked | idle
  deriving DecidableEq

def att0 (v : V) : Att × V :=
  match v.l0 with
  | [] => (.idle, v)
  | i :: rest =>
    let r := CrossingV1.Latch.step v.a i
    let m : Option SI := r.2.bind CrossingV1.Routes.latchToSignal
    if m.isSome ∧ 2 ≤ v.l1.length then (.blocked, v)
    else (.delivered, { v with a := r.1, l0 := rest, l1 := v.l1 ++ m.toList })

def att1 (v : V) : Att × V :=
  match v.l1 with
  | [] => (.idle, v)
  | i :: rest =>
    let r := CrossingV1.Signal.step v.b i
    let m : Option LI := r.2.bind CrossingV1.Routes.signalToLatch
    if m.isSome ∧ 2 ≤ v.l0.length then (.blocked, v)
    else (.delivered, { v with b := r.1, l1 := rest, l0 := v.l0 ++ m.toList })

def attempt (c : Nat) (v : V) : Att × V :=
  match c with
  | 0 => att0 v
  | 1 => att1 v
  | _ => (.idle, v)

def simOut (v : V) : Out :=
  let c := v.next % 2
  match attempt c v with
  | (.delivered, v1) => .delivered { v1 with next := (c + 1) % 2 }
  | (k, v1) =>
    let c1 := (v1.next + 1) % 2
    match attempt c1 v1 with
    | (.delivered, v2) => .delivered { v2 with next := (c1 + 1) % 2 }
    | (.blocked, _) => .blocked
    | (.idle, _) => match k with | .blocked => .blocked | _ => .settled

def lists2 {α} (xs : List α) : List (List α) :=
  [[]] ++ xs.map (fun x => [x]) ++ xs.flatMap (fun x => xs.map (fun y => [x, y]))

def allLS : List LS := [.idle, .pending]
def allSS : List SS := [.vehiclesGreen, .vehiclesAmber, .walk, .walkQueued, .clearance, .clearanceQueued]
def allLI : List LI := [.buttonPressed, .acknowledged]
def allSI : List SI := [.crossingRequested, .timerExpired]

def allV : List V :=
  allLS.flatMap fun a => allSS.flatMap fun b => (lists2 allLI).flatMap fun l0 => (lists2 allSI).flatMap fun l1 =>
    [0, 1].map fun n => ⟨a, b, l0, l1, n⟩

set_option maxRecDepth 100000 in
theorem model_agrees : allV.all (fun v => decide (leanOut v = simOut v)) = true := by
  decide +kernel


/-- The invariant of a scheduler state: mailboxes within capacity, cursor in range. -/
def WFv (v : V) : Prop := v.l0.length ≤ 2 ∧ v.l1.length ≤ 2 ∧ v.next < 2

theorem mem_lists2 {α} (xs : List α) (hxs : ∀ x, x ∈ xs) (l : List α) (h : l.length ≤ 2) : l ∈ lists2 xs := by
  rcases l with _ | ⟨x, _ | ⟨y, _ | ⟨z, w⟩⟩⟩
  · simp [lists2]
  · simp [lists2, hxs]
  · simp only [lists2, List.mem_append, List.mem_flatMap, List.mem_map]
    exact Or.inr ⟨x, hxs x, y, hxs y, rfl⟩
  · simp at h

theorem mem_allV (v : V) (h : WFv v) : v ∈ allV := by
  obtain ⟨h0, h1, h2⟩ := h
  rcases v with ⟨a, b, l0, l1, n⟩
  simp only at h0 h1 h2
  have hL : ∀ x : LI, x ∈ allLI := by intro x; cases x <;> simp [allLI]
  have hS : ∀ x : SI, x ∈ allSI := by intro x; cases x <;> simp [allSI]
  have ha : a ∈ allLS := by cases a <;> simp [allLS]
  have hb : b ∈ allSS := by cases b <;> simp [allSS]
  have hn : n ∈ ([0, 1] : List Nat) := by simp; omega
  simp only [allV, List.mem_flatMap, List.mem_map]
  exact ⟨a, ha, b, hb, l0, mem_lists2 _ hL _ h0, l1, mem_lists2 _ hS _ h1, n, hn, rfl⟩

/-- **The scheduler model agrees with `simOut` on every well-formed state.** Kernel `decide`, no `native_decide`. -/
theorem leanOut_eq (v : V) (h : WFv v) : leanOut v = simOut v := by
  have := List.all_eq_true.mp model_agrees v (mem_allV v h)
  simpa using this

def leanSend0 (v : V) : Option V :=
  match RoundRobin.send N (toSched v) ⟨0, by decide⟩ (CrossingV1.Ports.ButtonEvent.pressed : N.Msg ⟨0, by decide⟩) with
  | some s' => some (ofSched s')
  | none => none

def leanSend1 (v : V) : Option V :=
  match RoundRobin.send N (toSched v) ⟨1, by decide⟩ (CrossingV1.Ports.TimerEvent.expired : N.Msg ⟨1, by decide⟩) with
  | some s' => some (ofSched s')
  | none => none

def simSend0 (v : V) : Option V :=
  if v.l0.length < 2 then some { v with l0 := v.l0 ++ [.buttonPressed] } else none

def simSend1 (v : V) : Option V :=
  if v.l1.length < 2 then some { v with l1 := v.l1 ++ [.timerExpired] } else none

set_option maxRecDepth 100000 in
theorem send_agrees : allV.all (fun v => decide (leanSend0 v = simSend0 v) && decide (leanSend1 v = simSend1 v)) = true := by
  decide +kernel

theorem leanSend0_eq (v : V) (h : WFv v) : leanSend0 v = simSend0 v := by
  have := List.all_eq_true.mp send_agrees v (mem_allV v h)
  simp at this; exact this.1

theorem leanSend1_eq (v : V) (h : WFv v) : leanSend1 v = simSend1 v := by
  have := List.all_eq_true.mp send_agrees v (mem_allV v h)
  simp at this; exact this.2

/-- `toSched` is injective on what `ofSched` can see: the scheduler is determined by its view. -/
theorem toSched_ofSched (s : RoundRobin.Sched N) : toSched (ofSched s) = s := by
  rcases s with ⟨⟨st, ib⟩, nx⟩
  simp only [toSched, ofSched, mkCfg, RoundRobin.Sched.mk.injEq, and_true]
  congr 1 <;> funext d <;> rcases d with ⟨_ | _ | n, hd⟩ <;> first | rfl | (exfalso; change n + 2 < 2 at hd; omega)

theorem ofSched_toSched (v : V) : ofSched (toSched v) = v := rfl


/-- The initial scheduler data. -/
theorem toSched_start :
    toSched ⟨CrossingV1.Latch.init, CrossingV1.Signal.init, [], [], 0⟩ = RoundRobin.start N := by
  simp only [toSched, RoundRobin.start, initial, mkCfg, RoundRobin.Sched.mk.injEq, and_true]
  congr 1 <;> funext d <;> rcases d with ⟨_ | _ | n, hd⟩ <;> first | rfl | (exfalso; change n + 2 < 2 at hd; omega)

end AeneasV1
