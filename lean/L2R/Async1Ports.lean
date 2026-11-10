import L2R.Async1

/-!
# Output ports and the `composite/v2` scheduler (`l2r.async/v1`)

`composite/v2` composes machines hierarchically and lets a system *emit*
messages to its environment through output ports. Neither needs a new
semantics: the behaviour of a hierarchical model is that of the flat
`L2R.Async1.Network` of its machines, and an output port is a child of that
network.

## Output ports as children

An output port with capacity `n` is a child whose mailbox is the **outbox**: its
state is `Unit`, its input type is the port's message type, and it emits
nothing (`outboxStep`). Then, with no change to `L2R.Async1`:

- a machine output routed to the port is appended to the outbox by the sender's
  delivery, which is enabled only if the outbox has room (backpressure: a full
  outbox *blocks* the sender, and nothing is dropped);
- a `deliver` step of the outbox child removes the oldest message. That step is
  **the environment taking a message**. The semantics permit it at any time,
  so every property proved over all interleavings holds whenever, and in
  whatever order, the environment empties the outboxes, and also if it never does;
- every generic theorem of `L2R.Async1` (no overflow, invariants, fair
  termination) applies as it stands.

The children `0 … machines-1` of such a network are the machines; the outboxes
come after them.

## The scheduler

`Ports` models the `System` a root `composite/v2` generates. It differs from
`L2R.Async1.RoundRobin` in exactly one way: it schedules **only the machines**.
An outbox is emptied by the caller (`System::take_<port>`), never by
`step_once`. `run_sound` shows every run is a permitted behaviour of the
network, so system properties proved over `Reachable` hold for the generated
scheduler by inclusion.

**Trust:** the correspondence between `Ports` and the generated Rust is
established per project (refinement proof or conformance testing; see
`docs/verification.md`), not by this file.
-/
namespace L2R.Async1

/-- The transition of an outbox child: it holds messages and emits nothing. -/
def outboxStep (M : Type) : Unit → M → Unit × Option Empty := fun s _ => (s, none)

namespace Ports

variable (N : Network) (machines : Nat)

/-- `System`: the configuration and the round-robin cursor. -/
structure Sched where
  cfg : Config N
  next : Nat

/-- `System::new`. -/
def start : Sched N := ⟨initial N, 0⟩

/-- The children `System::step_once` tries, in order: machines only. -/
def order (next : Nat) : List (Fin N.size) :=
  (List.range machines).filterMap fun off =>
    if h : (next + off) % machines < N.size then some ⟨(next + off) % machines, h⟩ else none

/-- What `System::step_once` reports. -/
inductive Progress where
  | delivered (s : Sched N)
  | settled
  | blocked

/-- The delivery `System::step_once` performs, if any: the first machine in
round-robin order whose delivery is enabled, and the new cursor. -/
def pick (s : Sched N) : Option (Sched N) :=
  (order N machines s.next).findSome? fun c =>
    (deliver N s.cfg c).map fun k => (⟨k, (c.val + 1) % machines⟩ : Sched N)

/-- No machine's mailbox holds a message (outboxes may). -/
def quiet (k : Config N) : Bool :=
  (List.finRange N.size).all fun c => decide (machines ≤ c.val) || (k.inbox c).isEmpty

/-- `System::step_once`. -/
def stepOnce (s : Sched N) : Progress N :=
  match pick N machines s with
  | some s' => .delivered s'
  | none => if quiet N machines s.cfg then .settled else .blocked

/-- `System::send_<port>`: `none` is the explicit rejection (the target is full). -/
def send (s : Sched N) (p : Fin N.ports) (m : N.Msg p) : Option (Sched N) :=
  if (s.cfg.inbox (N.target p)).length < N.cap (N.target p) then
    some ⟨inject N s.cfg p m, s.next⟩
  else none

/-- `System::take_<port>` on the outbox child `c`: removes its oldest message.
`none` when the outbox is empty. The cursor is unchanged. -/
def take (s : Sched N) (c : Fin N.size) : Option (Sched N) :=
  (deliver N s.cfg c).map fun k => (⟨k, s.next⟩ : Sched N)

/-- Runs of the scheduler, with the caller's sends restricted to `E` and its
takes unrestricted. -/
inductive Run (E : Env N) : Sched N → Prop
  | start : Run E (start N)
  | step {s s' : Sched N} : Run E s → stepOnce N machines s = .delivered s' → Run E s'
  | send {s s' : Sched N} (p : Fin N.ports) (m : N.Msg p) :
      Run E s → E s.cfg p m → send N s p m = some s' → Run E s'
  | take {s s' : Sched N} (c : Fin N.size) :
      Run E s → machines ≤ c.val → take N s c = some s' → Run E s'

/-- Every child `System::step_once` tries is a machine. -/
theorem order_machine (next : Nat) {c : Fin N.size} (h : c ∈ order N machines next) :
    c.val < machines := by
  unfold order at h
  rw [List.mem_filterMap] at h
  obtain ⟨off, hoff, hc⟩ := h
  have hpos : 0 < machines := Nat.lt_of_le_of_lt (Nat.zero_le _) (List.mem_range.mp hoff)
  split at hc
  · injection hc with hc
    subst hc
    exact Nat.mod_lt _ hpos
  · contradiction

/-- `System::step_once` tries every machine. -/
theorem mem_order (next : Nat) (c : Fin N.size) (hc : c.val < machines) :
    c ∈ order N machines next := by
  have hn : 0 < machines := Nat.lt_of_le_of_lt (Nat.zero_le _) hc
  have hval : (next + (c.val + machines - next % machines) % machines) % machines = c.val := by
    rw [Nat.add_mod_mod, ← Nat.mod_add_mod]
    have h1 : next % machines < machines := Nat.mod_lt _ hn
    generalize next % machines = r at *
    have : r + (c.val + machines - r) = c.val + machines := by omega
    rw [this, Nat.add_mod_right, Nat.mod_eq_of_lt hc]
  unfold order
  rw [List.mem_filterMap]
  refine ⟨(c.val + machines - next % machines) % machines, List.mem_range.mpr (Nat.mod_lt _ hn), ?_⟩
  split
  · exact congrArg some (Fin.ext hval)
  · rename_i hlt
    exact absurd (by rw [hval]; exact c.isLt) hlt

/-- What a scheduler step is: the delivery of one **machine**. -/
theorem stepOnce_delivers {s s' : Sched N} (h : stepOnce N machines s = .delivered s') :
    ∃ c : Fin N.size, c.val < machines ∧ deliver N s.cfg c = some s'.cfg := by
  unfold stepOnce at h
  split at h
  · rename_i s'' hs''
    injection h with h
    subst h
    unfold pick at hs''
    obtain ⟨c, hmem, hc⟩ := List.exists_of_findSome?_eq_some hs''
    cases hs : deliver N s.cfg c with
    | none => simp [hs] at hc
    | some k =>
      simp [hs] at hc
      subst hc
      exact ⟨c, order_machine N machines _ hmem, hs⟩
  · split at h <;> contradiction

/-- Every scheduler step is a permitted delivery. -/
theorem stepOnce_sound (E : Env N) {s s' : Sched N} (h : stepOnce N machines s = .delivered s') :
    Step N E s.cfg s'.cfg := by
  obtain ⟨c, _, hc⟩ := stepOnce_delivers N machines h
  exact ⟨.deliver c, StepL.deliver c hc⟩

theorem send_sound (E : Env N) {s s' : Sched N} (p : Fin N.ports) (m : N.Msg p)
    (he : E s.cfg p m) (h : send N s p m = some s') : Step N E s.cfg s'.cfg := by
  unfold send at h
  split at h
  · rename_i hroom
    injection h with h
    subst h
    exact ⟨.send p m, StepL.send _ p m he hroom⟩
  · contradiction

/-- Taking from an outbox is a permitted step: the delivery of the outbox child. -/
theorem take_sound (E : Env N) {s s' : Sched N} (c : Fin N.size)
    (h : take N s c = some s') : Step N E s.cfg s'.cfg := by
  unfold take at h
  cases hd : deliver N s.cfg c with
  | none => simp [hd] at h
  | some k =>
    simp [hd] at h
    subst h
    exact ⟨.deliver c, StepL.deliver c hd⟩

/-- **Trace inclusion.** Every configuration the scheduler reaches, under any
caller that sends only what `E` permits and takes from outboxes whenever it
likes, is reachable by permitted behaviour. -/
theorem run_sound (E : Env N) {s : Sched N} (h : Run N machines E s) : Reachable N E s.cfg := by
  induction h with
  | start => exact Reachable.init
  | step _ hs ih => exact Reachable.step ih (stepOnce_sound N machines E hs)
  | send p m _ he hsend ih => exact Reachable.step ih (send_sound N E p m he hsend)
  | take c _ _ htake ih => exact Reachable.step ih (take_sound N E c htake)

/-- Hence every system invariant proved over permitted behaviour holds for the scheduler. -/
theorem run_invariant (E : Env N) (P : Config N → Prop)
    (hP : ∀ k, Reachable N E k → P k) {s : Sched N} (h : Run N machines E s) : P s.cfg :=
  hP _ (run_sound N machines E h)

/-- The scheduler never overflows a mailbox or an outbox. -/
theorem run_bounded (E : Env N) {s : Sched N} (h : Run N machines E s) : Bounded N s.cfg :=
  (run_sound N machines E h).bounded N E

/-- A rejected send changes nothing and is reported. -/
theorem send_rejected_iff (s : Sched N) (p : Fin N.ports) (m : N.Msg p) :
    send N s p m = none ↔ N.cap (N.target p) ≤ (s.cfg.inbox (N.target p)).length := by
  unfold send
  split <;> simp_all <;> omega

theorem pick_isSome_iff (s : Sched N) :
    (pick N machines s).isSome ↔ ∃ c : Fin N.size, c.val < machines ∧ (deliver N s.cfg c).isSome := by
  unfold pick
  rw [List.findSome?_isSome_iff]
  constructor
  · rintro ⟨c, hmem, hc⟩
    exact ⟨c, order_machine N machines _ hmem, by cases hd : deliver N s.cfg c <;> simp [hd] at hc ⊢⟩
  · rintro ⟨c, hlt, hc⟩
    exact ⟨c, mem_order N machines s.next c hlt, by simpa using hc⟩

/-- **Progress, exactly.** The scheduler delivers whenever some machine's
delivery is enabled. -/
theorem stepOnce_delivered_iff (s : Sched N) :
    (∃ s', stepOnce N machines s = .delivered s') ↔
      ∃ c : Fin N.size, c.val < machines ∧ (deliver N s.cfg c).isSome := by
  rw [← pick_isSome_iff]
  unfold stepOnce
  constructor
  · rintro ⟨s', h⟩
    split at h
    · rename_i s'' hs''
      rw [hs'']; rfl
    · split at h <;> contradiction
  · intro h
    obtain ⟨s'', hs''⟩ := Option.isSome_iff_exists.mp h
    exact ⟨s'', by simp [hs'']⟩

theorem quiet_iff (k : Config N) :
    quiet N machines k = true ↔ ∀ c : Fin N.size, c.val < machines → k.inbox c = [] := by
  unfold quiet
  rw [List.all_eq_true]
  constructor
  · intro h c hc
    have := h c (List.mem_finRange c)
    simp only [Bool.or_eq_true, decide_eq_true_eq, List.isEmpty_iff] at this
    rcases this with h1 | h1
    · omega
    · exact h1
  · intro h c _
    simp only [Bool.or_eq_true, decide_eq_true_eq, List.isEmpty_iff]
    by_cases hc : c.val < machines
    · exact Or.inr (h c hc)
    · exact Or.inl (by omega)

/-- `Settled` is reported exactly when no machine's mailbox holds a message. -/
theorem stepOnce_settled_iff (s : Sched N) :
    stepOnce N machines s = .settled ↔ ∀ c : Fin N.size, c.val < machines → s.cfg.inbox c = [] := by
  have key := stepOnce_delivered_iff N machines s
  rw [← quiet_iff]
  unfold stepOnce at *
  constructor
  · intro h
    split at h
    · contradiction
    · split at h
      · assumption
      · contradiction
  · intro hq
    have hset := (quiet_iff N machines s.cfg).mp hq
    have hno : ¬ ∃ c : Fin N.size, c.val < machines ∧ (deliver N s.cfg c).isSome := by
      rintro ⟨c, hlt, hc⟩
      rw [deliver_empty N (hset c hlt)] at hc
      simp at hc
    split
    · rename_i s'' hs''
      exact absurd (key.mp ⟨s'', by simp [hs'']⟩) hno
    · simp [hq]

/-- `Blocked` is reported only when some machine has a pending message and no
machine's delivery is enabled: every pending delivery waits for a full mailbox
or for an outbox the environment has not emptied. -/
theorem stepOnce_blocked (s : Sched N) (h : stepOnce N machines s = .blocked) :
    (∃ c : Fin N.size, c.val < machines ∧ s.cfg.inbox c ≠ []) ∧
      ∀ c : Fin N.size, c.val < machines → deliver N s.cfg c = none := by
  refine ⟨?_, ?_⟩
  · refine Classical.byContradiction fun hno => ?_
    have hset : ∀ c : Fin N.size, c.val < machines → s.cfg.inbox c = [] := fun c hc =>
      Classical.byContradiction fun hne => hno ⟨c, hc, hne⟩
    have := (stepOnce_settled_iff N machines s).mpr hset
    rw [h] at this
    cases this
  · intro c hlt
    refine Classical.byContradiction fun hc => ?_
    have hsome : (deliver N s.cfg c).isSome := by
      cases hd : deliver N s.cfg c with
      | none => exact absurd hd hc
      | some _ => rfl
    obtain ⟨s', hs'⟩ := (stepOnce_delivered_iff N machines s).mpr ⟨c, hlt, hsome⟩
    rw [h] at hs'
    cases hs'

end Ports

end L2R.Async1
