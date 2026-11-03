/-!
# Asynchronous composition of owned state machines (`l2r.async/v0`)

This file is L2R's generic semantics of composition. It defines **what behaviour
an approved composition permits**, independently of any scheduler, and a model
of the scheduler that the `composite/v0` profile generates, with the theorem
that every run of that scheduler is a permitted behaviour.

It contains no behaviour of any particular system. A system is a `Network`
whose states, inputs, outputs and `step` functions are the Lean specification's
own declarations and whose `route` is built from the architecture model's
approved message edges and their Lean translation functions. L2R derives that
wiring mechanically (`l2r lean-composition`); nothing here restates it.

## Semantics, `l2r.async/v0`

- **State ownership.** Each child owns its state. Only a delivery to that child
  changes it, and only through the child's own `step`.
- **Mailboxes.** One FIFO mailbox per child. Messages to a child, from any
  sender and from the environment, are appended in the order they are produced.
- **Delivery is atomic (run to completion).** A delivery takes the oldest
  message of one child's mailbox, applies the child's `step`, and appends every
  message the output translates to, in approved-edge order.
- **Interleavings.** Any child with a non-empty mailbox may be served next, and
  the environment may send any input it is permitted to (`Env`) at any time.
  The permitted behaviours are exactly the sequences of these steps.
- **Not modelled in v0:** message loss, duplication or reordering; crashes;
  bounded mailboxes (capacity is a *property* to prove of a system, not a
  semantic limit); time. Each is a deliberate omission, not an assumption that
  they cannot happen in a deployment.

Executions orders and permitted behaviours are kept apart: `RoundRobin` is
*one* way to choose among permitted steps, and `RoundRobin.run_sound` shows that
it chooses only permitted ones. A system property proved over `Reachable`
therefore holds for every scheduler shown sound this way.
-/
namespace L2R.Async

/-- Dependent update of a function indexed by `Fin n`. -/
def update {n : Nat} {β : Fin n → Type} (f : (c : Fin n) → β c) (c : Fin n) (v : β c) :
    (d : Fin n) → β d :=
  fun d => if h : d = c then h ▸ v else f d

@[simp] theorem update_same {n : Nat} {β : Fin n → Type} (f : (c : Fin n) → β c) (c : Fin n)
    (v : β c) : update f c v c = v := by
  simp [update]

@[simp] theorem update_other {n : Nat} {β : Fin n → Type} (f : (c : Fin n) → β c) {c d : Fin n}
    (v : β c) (h : d ≠ c) : update f c v d = f d := by
  simp [update, h]

/-- An approved composition: children `0 … size-1` in identifier order. -/
structure Network where
  size : Nat
  State : Fin size → Type
  Input : Fin size → Type
  Output : Fin size → Type
  init : (c : Fin size) → State c
  step : (c : Fin size) → State c → Input c → State c × Option (Output c)
  /-- The messages one output becomes, in approved-edge order. -/
  route : (c : Fin size) → Output c → List ((d : Fin size) × Input d)

variable (N : Network)

/-- Every child's owned state and mailbox. -/
structure Config where
  state : (c : Fin N.size) → N.State c
  inbox : (c : Fin N.size) → List (N.Input c)

/-- Every child in its initial state, with empty mailboxes. -/
def initial : Config N := ⟨N.init, fun _ => []⟩

/-- Appends one message to its receiver's mailbox. -/
def post (k : Config N) (m : (d : Fin N.size) × N.Input d) : Config N :=
  ⟨k.state, update k.inbox m.1 (k.inbox m.1 ++ [m.2])⟩

/-- The messages a (possibly absent) output becomes. -/
def emitted (c : Fin N.size) : Option (N.Output c) → List ((d : Fin N.size) × N.Input d)
  | none => []
  | some o => N.route c o

/-- Atomic delivery of the oldest message in `c`'s mailbox; `none` if it is empty. -/
def deliver (k : Config N) (c : Fin N.size) : Option (Config N) :=
  match k.inbox c with
  | [] => none
  | i :: rest =>
    let r := N.step c (k.state c) i
    some ((emitted N c r.2).foldl (post N) ⟨update k.state c r.1, update k.inbox c rest⟩)

/-- The messages in `ms` addressed to `d`, in order. -/
def msgsFor (d : Fin N.size) (ms : List ((d : Fin N.size) × N.Input d)) : List (N.Input d) :=
  ms.filterMap fun m => if h : m.1 = d then some (h ▸ m.2) else none

theorem foldl_post_state (k : Config N) (ms : List ((d : Fin N.size) × N.Input d)) :
    (ms.foldl (post N) k).state = k.state := by
  induction ms generalizing k with
  | nil => rfl
  | cons m ms ih => simp only [List.foldl]; rw [ih]; rfl

theorem foldl_post_inbox (k : Config N) (ms : List ((d : Fin N.size) × N.Input d)) (d : Fin N.size) :
    (ms.foldl (post N) k).inbox d = k.inbox d ++ msgsFor N d ms := by
  induction ms generalizing k with
  | nil => simp [msgsFor]
  | cons m ms ih =>
    simp only [List.foldl]
    rw [ih]
    obtain ⟨e, i⟩ := m
    by_cases h : e = d
    · subst h
      simp [post, msgsFor]
    · have h' : d ≠ e := fun x => h x.symm
      simp [post, msgsFor, h, update_other _ _ h']

/-- What a delivery does, observed child by child: only `c`'s state changes, by
its own `step`; `c` loses its oldest message; each child receives, in order, the
messages addressed to it. -/
theorem deliver_some {k k' : Config N} {c : Fin N.size} (h : deliver N k c = some k') :
    ∃ i rest, k.inbox c = i :: rest ∧
      k'.state = update k.state c (N.step c (k.state c) i).1 ∧
      ∀ d, k'.inbox d = update k.inbox c rest d ++ msgsFor N d (emitted N c (N.step c (k.state c) i).2) := by
  unfold deliver at h
  split at h
  · contradiction
  · rename_i i rest hin
    injection h with h
    subst h
    exact ⟨i, rest, hin, foldl_post_state N _ _, fun d => foldl_post_inbox N _ _ d⟩

theorem deliver_empty {k : Config N} {c : Fin N.size} (h : k.inbox c = []) : deliver N k c = none := by
  unfold deliver; rw [h]

theorem post_state (k : Config N) (m : (d : Fin N.size) × N.Input d) : (post N k m).state = k.state := rfl

theorem post_inbox_same (k : Config N) (c : Fin N.size) (i : N.Input c) :
    (post N k ⟨c, i⟩).inbox c = k.inbox c ++ [i] := by
  simp [post]

theorem post_inbox_other (k : Config N) {c d : Fin N.size} (i : N.Input c) (h : d ≠ c) :
    (post N k ⟨c, i⟩).inbox d = k.inbox d := by
  simp [post, update_other _ _ h]

/-- The external inputs the environment may send. A system property is proved
for a stated environment; `anyInput` is what the `composite/v0` API accepts. -/
abbrev Env := (c : Fin N.size) → N.Input c → Prop

/-- The environment may send any input to any child. -/
def anyInput : Env N := fun _ _ => True

/-- One permitted step: a delivery to any child with pending input, or a send
the environment is permitted to make. -/
inductive Step (E : Env N) : Config N → Config N → Prop
  | deliver {k k' : Config N} (c : Fin N.size) : deliver N k c = some k' → Step E k k'
  | send (k : Config N) (c : Fin N.size) (i : N.Input c) : E c i → Step E k (post N k ⟨c, i⟩)

/-- Configurations some permitted behaviour reaches. -/
inductive Reachable (E : Env N) : Config N → Prop
  | init : Reachable E (initial N)
  | step {k k' : Config N} : Reachable E k → Step N E k k' → Reachable E k'

/-- `k'` follows `k` by permitted steps. -/
inductive Star (E : Env N) : Config N → Config N → Prop
  | refl (k : Config N) : Star E k k
  | tail {k k' k'' : Config N} : Star E k k' → Step N E k' k'' → Star E k k''

/-- An inductive invariant of the permitted steps holds in every reachable configuration. -/
theorem reachable_invariant (E : Env N) (P : Config N → Prop) (h0 : P (initial N))
    (hs : ∀ k k', Reachable N E k → P k → Step N E k k' → P k') :
    ∀ k, Reachable N E k → P k := by
  intro k hk
  induction hk with
  | init => exact h0
  | step hr hst ih => exact hs _ _ hr ih hst

/-- A property preserved by every permitted step is kept by every continuation. -/
theorem star_invariant (E : Env N) (P : Config N → Prop)
    (hs : ∀ k k', P k → Step N E k k' → P k') :
    ∀ k k', P k → Star N E k k' → P k' := by
  intro k k' hk hstar
  induction hstar with
  | refl => exact hk
  | tail _ hst ih => exact hs _ _ ih hst

/-!
## The `composite/v0` scheduler

A model of the generated `System` (one composite whose children are all
machines), written to mirror the generated Rust rather than reuse `deliver`:

- `System::step_once` tries `child = (next + offset) % n` for `offset` in `0..n`
  and serves the first child whose mailbox is non-empty, then sets
  `next = (child + 1) % n`;
- `System::deliver` pops the front of the mailbox, applies `step`, stores the
  new state, and pushes each translated message to the back of its receiver's
  mailbox, edge by edge; an output with no approved edge is dropped;
- `System::send` pushes an envelope to the back of its addressee's mailbox.

**Trust:** the correspondence between this model and the generated Rust is
established by review of the profile, **not by proof**. See
`docs/verification.md`.
-/
namespace RoundRobin

/-- `System`: the configuration and the round-robin cursor. -/
structure Sched where
  cfg : Config N
  next : Nat

/-- `System::new`. -/
def start : Sched N := ⟨initial N, 0⟩

/-- The children `System::step_once` tries, in order. -/
def order (next : Nat) : List (Fin N.size) :=
  (List.range N.size).filterMap fun off =>
    if h : (next + off) % N.size < N.size then some ⟨(next + off) % N.size, h⟩ else none

/-- `push_back` of each translated message, edge by edge. -/
def pushAll : Config N → List ((d : Fin N.size) × N.Input d) → Config N
  | k, [] => k
  | k, ⟨d, i⟩ :: ms => pushAll ⟨k.state, update k.inbox d (k.inbox d ++ [i])⟩ ms

/-- `System::deliver(child)`. -/
def serve (k : Config N) (c : Fin N.size) : Option (Config N) :=
  match k.inbox c with
  | [] => none
  | input :: rest =>
    let (state, output) := N.step c (k.state c) input
    let k1 : Config N := ⟨update k.state c state, update k.inbox c rest⟩
    match output with
    | none => some k1
    | some o => some (pushAll N k1 (N.route c o))

/-- `System::step_once`; `none` when it returns `false`. -/
def stepOnce (s : Sched N) : Option (Sched N) :=
  (order N s.next).findSome? fun c => (serve N s.cfg c).map fun k => ⟨k, (c.val + 1) % N.size⟩

/-- `System::send`. -/
def send (s : Sched N) (c : Fin N.size) (i : N.Input c) : Sched N :=
  ⟨⟨s.cfg.state, update s.cfg.inbox c (s.cfg.inbox c ++ [i])⟩, s.next⟩

/-- Runs of the scheduler, with the caller's sends restricted to `E`. -/
inductive Run (E : Env N) : Sched N → Prop
  | start : Run E (start N)
  | step {s s' : Sched N} : Run E s → stepOnce N s = some s' → Run E s'
  | send {s : Sched N} (c : Fin N.size) (i : N.Input c) : Run E s → E c i → Run E (send N s c i)

theorem pushAll_eq_foldl (k : Config N) (ms : List ((d : Fin N.size) × N.Input d)) :
    pushAll N k ms = ms.foldl (post N) k := by
  induction ms generalizing k with
  | nil => rfl
  | cons m ms ih => cases m; simp [pushAll, ih, post, List.foldl]

/-- Serving a child is exactly the abstract atomic delivery. -/
theorem serve_eq_deliver (k : Config N) (c : Fin N.size) : serve N k c = deliver N k c := by
  unfold serve deliver
  split
  · rfl
  · rename_i i rest _
    cases h : N.step c (k.state c) i with
    | mk st out =>
      cases out with
      | none => simp [emitted]
      | some o => simp [emitted, pushAll_eq_foldl]

/-- Every scheduler step is a permitted delivery. -/
theorem stepOnce_sound (E : Env N) {s s' : Sched N} (h : stepOnce N s = some s') :
    Step N E s.cfg s'.cfg := by
  unfold stepOnce at h
  obtain ⟨c, _, hc⟩ := List.exists_of_findSome?_eq_some h
  cases hs : serve N s.cfg c with
  | none => simp [hs] at hc
  | some k =>
    simp [hs] at hc
    subst hc
    exact Step.deliver c (by rw [← serve_eq_deliver]; exact hs)

/-- **Trace inclusion.** Every configuration the scheduler reaches, under any
caller that sends only what `E` permits, is reachable by permitted behaviour. -/
theorem run_sound (E : Env N) {s : Sched N} (h : Run N E s) : Reachable N E s.cfg := by
  induction h with
  | start => exact Reachable.init
  | step _ hs ih => exact Reachable.step ih (stepOnce_sound N E hs)
  | send c i _ he ih => exact Reachable.step ih (Step.send _ c i he)

/-- Hence every system invariant proved over permitted behaviour holds for the scheduler. -/
theorem run_invariant (E : Env N) (P : Config N → Prop)
    (hP : ∀ k, Reachable N E k → P k) {s : Sched N} (h : Run N E s) : P s.cfg :=
  hP _ (run_sound N E h)

/-- `System::step_once` tries every child. -/
theorem mem_order (next : Nat) (c : Fin N.size) : c ∈ order N next := by
  have hn : 0 < N.size := Nat.lt_of_le_of_lt (Nat.zero_le _) c.isLt
  have hval : (next + (c.val + N.size - next % N.size) % N.size) % N.size = c.val := by
    rw [Nat.add_mod_mod, ← Nat.mod_add_mod]
    have h1 : next % N.size < N.size := Nat.mod_lt _ hn
    generalize next % N.size = r at *
    have : r + (c.val + N.size - r) = c.val + N.size := by omega
    rw [this, Nat.add_mod_right, Nat.mod_eq_of_lt c.isLt]
  unfold order
  rw [List.mem_filterMap]
  refine ⟨(c.val + N.size - next % N.size) % N.size, List.mem_range.mpr (Nat.mod_lt _ hn), ?_⟩
  split
  · exact congrArg some (Fin.ext hval)
  · rename_i hlt
    exact absurd (by rw [hval]; exact c.isLt) hlt

theorem serve_none_iff (k : Config N) (c : Fin N.size) : serve N k c = none ↔ k.inbox c = [] := by
  rw [serve_eq_deliver]
  unfold deliver
  split <;> simp_all

/-- **Progress.** The scheduler stops only when every mailbox is empty: it never
idles while a permitted delivery exists. -/
theorem stepOnce_none_iff (s : Sched N) : stepOnce N s = none ↔ ∀ c, s.cfg.inbox c = [] := by
  unfold stepOnce
  rw [List.findSome?_eq_none_iff]
  constructor
  · intro h c
    have := h c (mem_order N s.next c)
    rw [Option.map_eq_none_iff, serve_none_iff] at this
    exact this
  · intro h c _
    rw [Option.map_eq_none_iff, serve_none_iff]
    exact h c

end RoundRobin

end L2R.Async
