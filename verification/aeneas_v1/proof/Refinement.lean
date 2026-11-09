import Extracted.CrossingV1Core
import AeneasV1.Model

/-!
# The generated `composite/v1` Rust scheduler refines the Lean scheduler model

`Extracted.CrossingV1Core` is the Aeneas translation of the committed, *generated* composite of
`examples/crossing_v1` (`System`, the array-backed mailboxes, the straight-line `step_once`, the
`send_*` ports, both machines and the message translations). It is regenerated from those exact
sources by `//verification/aeneas_v1:refinement_test` (and must equal the committed copy).

Under the abstraction `abs : System → RoundRobin.Sched N` (below; `N` is the derived network
`L2RDerived.crossing_v1.crossing.network`) and the invariant `Inv`, this file proves, without any
panic (`= ok r`):

* `new_refines`: `System::new` yields a well-formed system that is `RoundRobin.start N`;
* `step_once_refines`: `step_once` reports `Delivered` with exactly the successor of
  `RoundRobin.stepOnce`, or `Settled` / `Blocked` (system unchanged), iff the model does;
* `send_button_refines`, `send_timer_refines`: `Ok(())` with exactly the successor of
  `RoundRobin.send`, or `Err(Rejected)` (system unchanged), iff the model does;
* `rustRun_simulates`, `rustRun_bounded`: every system reachable through the generated API is a
  `RoundRobin.Run` state under the most permissive environment, so the model's run theorems hold.

Proof structure: mailbox lemmas (`*_push_spec`, `*_pop_spec`, ...), the per-child `attempt_*` steps,
`attempt`, then `step_once`, all against the plain functions `att0`, `att1`, `simOut`, `simSend*` of
`AeneasV1/Model.lean`, which that file proves equal to the model on every well-formed state.

Trusted base: the Lean kernel; Charon and Aeneas translating faithfully; rustc; the abstraction
functions (pinned and reviewed in `expected-statements.txt`); the derived network and
`lean/L2R/Async1.lean` (inputs, pinned by their use in the audited statements).
-/

set_option linter.unusedSimpArgs false
set_option linter.unusedTactic false
set_option linter.unusedVariables false
open Aeneas Aeneas.Std Result
open crossing_v1_core
open L2R.Async1 AeneasV1
namespace AeneasV1.Refinement

theorem spec_ok_exists {α} {m : Result α} {p : α → Prop} (h : WP.spec m p) : ∃ v, m = ok v ∧ p v := by
  cases m with
  | ret a => exact ⟨a, rfl, by simpa using h⟩
  | vis e k => simp at h
  | div => simp at h

def latchInput : crossing.request_latch.Input → CrossingV1.Latch.Input
  | .ButtonPressed => .buttonPressed
  | .Acknowledged => .acknowledged
def signalInput : crossing.signal.Input → CrossingV1.Signal.Input
  | .CrossingRequested => .crossingRequested
  | .TimerExpired => .timerExpired

/-! ### Mailbox `RequestLatchInbox` -/

/-- `b` is a well-formed mailbox holding exactly the messages `l`, oldest first: `len = |l| ≤ 2`,
the first `|l|` slots are `some`, the rest `none`. -/
def RepL (b : crossing.RequestLatchInbox) (l : List crossing.request_latch.Input) : Prop :=
  b.len.val = l.length ∧ l.length ≤ 2 ∧ b.slots.val = l.map some ++ List.replicate (2 - l.length) none

/-- The messages a mailbox holds, as the Lean specification sees them. -/
def absL (b : crossing.RequestLatchInbox) : List CrossingV1.Latch.Input :=
  (b.slots.val.filterMap id).map latchInput

theorem absL_of_rep {b l} (h : RepL b l) : absL b = l.map latchInput := by
  obtain ⟨h1, h2, h3⟩ := h
  simp only [absL, h3, List.filterMap_append]
  have : (List.replicate (2 - l.length) (none : Option crossing.request_latch.Input)).filterMap id = [] := by
    simp
  rw [this]
  simp [List.filterMap_map]

theorem latchbox_new_spec : crossing.RequestLatchInbox.new ⦃ b => RepL b [] ⦄ := by
  simp [crossing.RequestLatchInbox.new, RepL]

theorem latchbox_is_empty_spec {b l} (h : RepL b l) :
    crossing.RequestLatchInbox.is_empty b ⦃ r => r = decide (l.length = 0) ⦄ := by
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.RequestLatchInbox.is_empty
  simp only [WP.spec_ok]
  by_cases hl : l.length = 0
  · have : b.len = 0#usize := by scalar_tac
    simp [this, hl]
  · have : ¬ b.len = 0#usize := by scalar_tac
    simp [this, hl]

theorem latchbox_is_full_spec {b l} (h : RepL b l) :
    crossing.RequestLatchInbox.is_full b ⦃ r => r = decide (2 ≤ l.length) ⦄ := by
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.RequestLatchInbox.is_full
  simp only [WP.spec_ok, crossing.REQUEST_LATCH_CAPACITY]
  by_cases hl : 2 ≤ l.length
  · have : b.len ≥ 2#usize := by scalar_tac
    simp [this, hl]
  · have : ¬ b.len ≥ 2#usize := by scalar_tac
    simp [this, hl]

theorem latchbox_peek_spec {b l} (h : RepL b l) :
    crossing.RequestLatchInbox.peek b ⦃ r => r = l.head? ⦄ := by
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.RequestLatchInbox.peek
  step as ⟨x, hx⟩
  rcases l with _ | ⟨y, _⟩
  · simp [hx, h3]
  · simp [hx, h3]

theorem latchbox_push_spec {b l} (i : crossing.request_latch.Input) (h : RepL b l) :
    crossing.RequestLatchInbox.push b i ⦃ r => r.1 = decide (l.length < 2) ∧
      RepL r.2 (if l.length < 2 then l ++ [i] else l) ∧ (2 ≤ l.length → r.2 = b) ⦄ := by
  have h' := h
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.RequestLatchInbox.push
  by_cases hl : l.length < 2
  · have : ¬ b.len ≥ crossing.REQUEST_LATCH_CAPACITY := by
      simp [crossing.REQUEST_LATCH_CAPACITY]
      scalar_tac
    simp only [this, ite_false]
    step as ⟨a, ha⟩
    step as ⟨n, hn⟩
    simp only [hl, decide_true, ite_true, RepL, true_and]
    subst ha
    rcases l with _ | ⟨x, _ | ⟨y, l⟩⟩
    · simp_all
    · simp_all
    · simp at hl
  · have : b.len ≥ crossing.REQUEST_LATCH_CAPACITY := by
      simp [crossing.REQUEST_LATCH_CAPACITY]
      scalar_tac
    simp [this, hl, h']

theorem latchbox_pop_spec {b l} (h : RepL b l) :
    crossing.RequestLatchInbox.pop b ⦃ r => r.1 = l.head? ∧ RepL r.2 l.tail ⦄ := by
  have h' := h
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.RequestLatchInbox.pop
  rcases l with _ | ⟨x, _ | ⟨y, _ | ⟨z, w⟩⟩⟩
  · have : b.len = 0#usize := by scalar_tac
    simp [this, h']
  · have : ¬ b.len = 0#usize := by scalar_tac
    simp only [this, ite_false]
    step as ⟨first, hf⟩
    step as ⟨o, ho⟩
    step as ⟨a, ha⟩
    step as ⟨a1, ha1⟩
    step as ⟨n, hn⟩
    subst ha1 ha
    simp_all [RepL]
  · have : ¬ b.len = 0#usize := by scalar_tac
    simp only [this, ite_false]
    step as ⟨first, hf⟩
    step as ⟨o, ho⟩
    step as ⟨a, ha⟩
    step as ⟨a1, ha1⟩
    step as ⟨n, hn⟩
    subst ha1 ha
    simp_all [RepL]
  · simp at h2

/-! ### Mailbox `SignalInbox` -/

/-- `b` is a well-formed mailbox holding exactly the messages `l`, oldest first: `len = |l| ≤ 2`,
the first `|l|` slots are `some`, the rest `none`. -/
def RepS (b : crossing.SignalInbox) (l : List crossing.signal.Input) : Prop :=
  b.len.val = l.length ∧ l.length ≤ 2 ∧ b.slots.val = l.map some ++ List.replicate (2 - l.length) none

/-- The messages a mailbox holds, as the Lean specification sees them. -/
def absS (b : crossing.SignalInbox) : List CrossingV1.Signal.Input :=
  (b.slots.val.filterMap id).map signalInput

theorem absS_of_rep {b l} (h : RepS b l) : absS b = l.map signalInput := by
  obtain ⟨h1, h2, h3⟩ := h
  simp only [absS, h3, List.filterMap_append]
  have : (List.replicate (2 - l.length) (none : Option crossing.signal.Input)).filterMap id = [] := by
    simp
  rw [this]
  simp [List.filterMap_map]

theorem signalbox_new_spec : crossing.SignalInbox.new ⦃ b => RepS b [] ⦄ := by
  simp [crossing.SignalInbox.new, RepS]

theorem signalbox_is_empty_spec {b l} (h : RepS b l) :
    crossing.SignalInbox.is_empty b ⦃ r => r = decide (l.length = 0) ⦄ := by
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.SignalInbox.is_empty
  simp only [WP.spec_ok]
  by_cases hl : l.length = 0
  · have : b.len = 0#usize := by scalar_tac
    simp [this, hl]
  · have : ¬ b.len = 0#usize := by scalar_tac
    simp [this, hl]

theorem signalbox_is_full_spec {b l} (h : RepS b l) :
    crossing.SignalInbox.is_full b ⦃ r => r = decide (2 ≤ l.length) ⦄ := by
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.SignalInbox.is_full
  simp only [WP.spec_ok, crossing.SIGNAL_CAPACITY]
  by_cases hl : 2 ≤ l.length
  · have : b.len ≥ 2#usize := by scalar_tac
    simp [this, hl]
  · have : ¬ b.len ≥ 2#usize := by scalar_tac
    simp [this, hl]

theorem signalbox_peek_spec {b l} (h : RepS b l) :
    crossing.SignalInbox.peek b ⦃ r => r = l.head? ⦄ := by
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.SignalInbox.peek
  step as ⟨x, hx⟩
  rcases l with _ | ⟨y, _⟩
  · simp [hx, h3]
  · simp [hx, h3]

theorem signalbox_push_spec {b l} (i : crossing.signal.Input) (h : RepS b l) :
    crossing.SignalInbox.push b i ⦃ r => r.1 = decide (l.length < 2) ∧
      RepS r.2 (if l.length < 2 then l ++ [i] else l) ∧ (2 ≤ l.length → r.2 = b) ⦄ := by
  have h' := h
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.SignalInbox.push
  by_cases hl : l.length < 2
  · have : ¬ b.len ≥ crossing.SIGNAL_CAPACITY := by
      simp [crossing.SIGNAL_CAPACITY]
      scalar_tac
    simp only [this, ite_false]
    step as ⟨a, ha⟩
    step as ⟨n, hn⟩
    simp only [hl, decide_true, ite_true, RepS, true_and]
    subst ha
    rcases l with _ | ⟨x, _ | ⟨y, l⟩⟩
    · simp_all
    · simp_all
    · simp at hl
  · have : b.len ≥ crossing.SIGNAL_CAPACITY := by
      simp [crossing.SIGNAL_CAPACITY]
      scalar_tac
    simp [this, hl, h']

theorem signalbox_pop_spec {b l} (h : RepS b l) :
    crossing.SignalInbox.pop b ⦃ r => r.1 = l.head? ∧ RepS r.2 l.tail ⦄ := by
  have h' := h
  obtain ⟨h1, h2, h3⟩ := h
  unfold crossing.SignalInbox.pop
  rcases l with _ | ⟨x, _ | ⟨y, _ | ⟨z, w⟩⟩⟩
  · have : b.len = 0#usize := by scalar_tac
    simp [this, h']
  · have : ¬ b.len = 0#usize := by scalar_tac
    simp only [this, ite_false]
    step as ⟨first, hf⟩
    step as ⟨o, ho⟩
    step as ⟨a, ha⟩
    step as ⟨a1, ha1⟩
    step as ⟨n, hn⟩
    subst ha1 ha
    simp_all [RepS]
  · have : ¬ b.len = 0#usize := by scalar_tac
    simp only [this, ite_false]
    step as ⟨first, hf⟩
    step as ⟨o, ho⟩
    step as ⟨a, ha⟩
    step as ⟨a1, ha1⟩
    step as ⟨n, hn⟩
    subst ha1 ha
    simp_all [RepS]
  · simp at h2


/-! ### Machines and message translations -/

instance : Nonempty crossing.request_latch.Output := ⟨.Requested⟩
instance : Nonempty crossing.signal.Output := ⟨.WalkStarted⟩

def latchState : crossing.request_latch.State → CrossingV1.Latch.State
  | .Idle => .idle
  | .Pending => .pending
def latchOutput : crossing.request_latch.Output → CrossingV1.Latch.Output
  | .Requested => .requested
def signalState : crossing.signal.State → CrossingV1.Signal.State
  | .VehiclesGreen => .vehiclesGreen
  | .VehiclesAmber => .vehiclesAmber
  | .Walk => .walk
  | .WalkQueued => .walkQueued
  | .Clearance => .clearance
  | .ClearanceQueued => .clearanceQueued
def signalOutput : crossing.signal.Output → CrossingV1.Signal.Output
  | .WalkStarted => .walkStarted

theorem latch_step_spec (s : crossing.request_latch.State) (i : crossing.request_latch.Input) :
    crossing.request_latch.step s i ⦃ r =>
      (latchState r.1, r.2.map latchOutput) = CrossingV1.Latch.step (latchState s) (latchInput i) ⦄ := by
  cases s <;> cases i <;>
    simp [crossing.request_latch.step, crossing.request_latch.imp.step, CrossingV1.Latch.step,
      latchState, latchInput, latchOutput]

theorem signal_step_spec (s : crossing.signal.State) (i : crossing.signal.Input) :
    crossing.signal.step s i ⦃ r =>
      (signalState r.1, r.2.map signalOutput) = CrossingV1.Signal.step (signalState s) (signalInput i) ⦄ := by
  cases s <;> cases i <;>
    simp [crossing.signal.step, crossing.signal.imp.step, CrossingV1.Signal.step,
      signalState, signalInput, signalOutput]

theorem latch_to_signal_spec (o : crossing.request_latch.Output) :
    crossing.routes.latch_to_signal o ⦃ r =>
      r.map signalInput = CrossingV1.Routes.latchToSignal (latchOutput o) ⦄ := by
  cases o; simp [crossing.routes.latch_to_signal, CrossingV1.Routes.latchToSignal, latchOutput, signalInput]

theorem signal_to_latch_spec (o : crossing.signal.Output) :
    crossing.routes.signal_to_latch o ⦃ r =>
      r.map latchInput = CrossingV1.Routes.signalToLatch (signalOutput o) ⦄ := by
  cases o; simp [crossing.routes.signal_to_latch, CrossingV1.Routes.signalToLatch, signalOutput, latchInput]

/-- The routing `match` of `attempt_*`. -/
theorem latch_route_spec (out : Option crossing.request_latch.Output) :
    (match out with
      | none => ok none
      | some output1 => crossing.routes.latch_to_signal output1) ⦃ r =>
      r.map signalInput = (out.map latchOutput).bind CrossingV1.Routes.latchToSignal ⦄ := by
  cases out with
  | none => simp
  | some o => simpa using latch_to_signal_spec o

theorem signal_route_spec (out : Option crossing.signal.Output) :
    (match out with
      | none => ok none
      | some output1 => crossing.routes.signal_to_latch output1) ⦃ r =>
      r.map latchInput = (out.map signalOutput).bind CrossingV1.Routes.signalToLatch ⦄ := by
  cases out with
  | none => simp
  | some o => simpa using signal_to_latch_spec o

/-! ### The abstraction and the invariant -/

/-- Rust `System` to the scheduler data of the Lean model. -/
def absV (s : crossing.System) : V :=
  ⟨latchState s.request_latch, signalState s.signal, absL s.request_latch_inbox, absS s.signal_inbox, s.next.val⟩

/-- Well-formedness: both mailboxes hold at most 2 messages in canonical form, and the cursor is a child index. -/
def Inv (s : crossing.System) : Prop :=
  (∃ l, RepL s.request_latch_inbox l) ∧ (∃ l, RepS s.signal_inbox l) ∧ s.next.val < 2

def attAbs : crossing.Attempt → Att
  | .Delivered => .delivered
  | .Blocked => .blocked
  | .Idle => .idle

theorem inv_wf {s} (h : Inv s) : WFv (absV s) := by
  obtain ⟨⟨l0, h0⟩, ⟨l1, h1⟩, hn⟩ := h
  refine ⟨?_, ?_, hn⟩
  · simp only [absV, absL_of_rep h0, List.length_map]; exact h0.2.1
  · simp only [absV, absS_of_rep h1, List.length_map]; exact h1.2.1

/-! ### `attempt_request_latch` -/

theorem attempt_request_latch_spec (s : crossing.System) (h : Inv s) :
    crossing.System.attempt_request_latch s ⦃ r =>
      attAbs r.1 = (att0 (absV s)).1 ∧ absV r.2 = (att0 (absV s)).2 ∧ Inv r.2 ∧
      (attAbs r.1 ≠ .delivered → r.2 = s) ⦄ := by
  have hinv := h
  obtain ⟨⟨l0, h0⟩, ⟨l1, h1⟩, hn⟩ := h
  unfold crossing.System.attempt_request_latch
  step with latchbox_peek_spec h0 as ⟨o, ho⟩
  rcases l0 with _ | ⟨i, rest⟩
  · simp [ho, attAbs, att0, absV, absL_of_rep h0, hinv]
  · subst ho
    simp only [List.head?_cons]
    have e0 : absL s.request_latch_inbox = latchInput i :: rest.map latchInput := by
      rw [absL_of_rep h0]; rfl
    have hatt : att0 (absV s) = (let r := CrossingV1.Latch.step (latchState s.request_latch) (latchInput i);
        let m : Option CrossingV1.Signal.Input := r.2.bind CrossingV1.Routes.latchToSignal;
        if m.isSome ∧ 2 ≤ (absS s.signal_inbox).length then (Att.blocked, absV s)
        else (Att.delivered, { absV s with a := r.1, l0 := rest.map latchInput, l1 := absS s.signal_inbox ++ m.toList })) := by
      simp [att0, absV, e0]
    rw [hatt]
    step with latch_step_spec s.request_latch i as ⟨st, out, hst⟩
    rw [← hst]
    step with (latch_route_spec out) as ⟨msg, hmsg⟩
    rw [← hmsg]
    have hlen : (absS s.signal_inbox).length = l1.length := by rw [absS_of_rep h1]; simp
    cases msg with
    | none =>
      simp only [Option.isSome_none, Option.map_none, Option.toList_none, Bool.false_eq_true, ite_false, false_and, List.append_nil]
      step with latchbox_pop_spec h0 as ⟨u, rli, hp1, hp2⟩
      simp only [List.tail_cons] at hp2
      refine ⟨rfl, ?_, ⟨⟨rest, hp2⟩, ⟨l1, h1⟩, hn⟩, by simp [attAbs]⟩
      simp [absV, absL_of_rep hp2]
    | some m =>
      simp only [Option.isSome_some, Option.map_some, Option.toList_some, true_and, ite_true, hlen]
      step with signalbox_is_full_spec h1 as ⟨b1, hb1⟩
      by_cases hf : 2 ≤ l1.length
      · have : b1 = true := by simp [hb1, hf]
        simp only [this, hf, ite_true, ite_false, WP.spec_ok]
        simp [attAbs, hinv]
      · have : b1 = false := by simp [hb1, hf]
        simp only [this, hf, ite_false, Bool.false_eq_true]
        step with latchbox_pop_spec h0 as ⟨u, rli, hp1, hp2⟩
        step with signalbox_push_spec m h1 as ⟨u2, si, hq1, hq2⟩
        simp only [List.tail_cons] at hp2
        have hlt : l1.length < 2 := by omega
        simp only [hlt, ite_true] at hq2
        refine ⟨rfl, ?_, ⟨⟨rest, hp2⟩, ⟨_, hq2⟩, hn⟩, by simp [attAbs]⟩
        simp [absV, absL_of_rep hp2, absS_of_rep hq2, absS_of_rep h1]

/-! ### `attempt_signal` -/

theorem attempt_signal_spec (s : crossing.System) (h : Inv s) :
    crossing.System.attempt_signal s ⦃ r =>
      attAbs r.1 = (att1 (absV s)).1 ∧ absV r.2 = (att1 (absV s)).2 ∧ Inv r.2 ∧
      (attAbs r.1 ≠ .delivered → r.2 = s) ⦄ := by
  have hinv := h
  obtain ⟨⟨l0, h0⟩, ⟨l1, h1⟩, hn⟩ := h
  unfold crossing.System.attempt_signal
  step with signalbox_peek_spec h1 as ⟨o, ho⟩
  rcases l1 with _ | ⟨i, rest⟩
  · simp [ho, attAbs, att1, absV, absS_of_rep h1, hinv]
  · subst ho
    simp only [List.head?_cons]
    have e0 : absS s.signal_inbox = signalInput i :: rest.map signalInput := by
      rw [absS_of_rep h1]; rfl
    have hatt : att1 (absV s) = (let r := CrossingV1.Signal.step (signalState s.signal) (signalInput i);
        let m : Option CrossingV1.Latch.Input := r.2.bind CrossingV1.Routes.signalToLatch;
        if m.isSome ∧ 2 ≤ (absL s.request_latch_inbox).length then (Att.blocked, absV s)
        else (Att.delivered, { absV s with b := r.1, l1 := rest.map signalInput, l0 := absL s.request_latch_inbox ++ m.toList })) := by
      simp [att1, absV, e0]
    rw [hatt]
    step with signal_step_spec s.signal i as ⟨st, out, hst⟩
    rw [← hst]
    step with (signal_route_spec out) as ⟨msg, hmsg⟩
    rw [← hmsg]
    have hlen : (absL s.request_latch_inbox).length = l0.length := by rw [absL_of_rep h0]; simp
    cases msg with
    | none =>
      simp only [Option.isSome_none, Option.map_none, Option.toList_none, Bool.false_eq_true, ite_false, false_and, List.append_nil]
      step with signalbox_pop_spec h1 as ⟨u, rli, hp1, hp2⟩
      simp only [List.tail_cons] at hp2
      refine ⟨rfl, ?_, ⟨⟨l0, h0⟩, ⟨rest, hp2⟩, hn⟩, by simp [attAbs]⟩
      simp [absV, absS_of_rep hp2]
    | some m =>
      simp only [Option.isSome_some, Option.map_some, Option.toList_some, true_and, ite_true, hlen]
      step with latchbox_is_full_spec h0 as ⟨b1, hb1⟩
      by_cases hf : 2 ≤ l0.length
      · have : b1 = true := by simp [hb1, hf]
        simp only [this, hf, ite_true, ite_false, WP.spec_ok]
        simp [attAbs, hinv]
      · have : b1 = false := by simp [hb1, hf]
        simp only [this, hf, ite_false, Bool.false_eq_true]
        step with signalbox_pop_spec h1 as ⟨u, rli, hp1, hp2⟩
        step with latchbox_push_spec m h0 as ⟨u2, si, hq1, hq2⟩
        simp only [List.tail_cons] at hp2
        have hlt : l0.length < 2 := by omega
        simp only [hlt, ite_true] at hq2
        refine ⟨rfl, ?_, ⟨⟨_, hq2⟩, ⟨rest, hp2⟩, hn⟩, by simp [attAbs]⟩
        simp [absV, absS_of_rep hp2, absL_of_rep hq2, absL_of_rep h0]

/-! ### `attempt`, `step_once` -/

theorem attempt_spec (s : crossing.System) (c : Usize) (hc : c.val < 2) (h : Inv s) :
    crossing.System.attempt s c ⦃ r =>
      (attAbs r.1, absV r.2) = attempt c.val (absV s) ∧ Inv r.2 ∧ (attAbs r.1 ≠ .delivered → r.2 = s) ⦄ := by
  unfold crossing.System.attempt
  have : c.val = 0 ∨ c.val = 1 := by omega
  rcases this with h0 | h0
  · simp only [h0]
    have := attempt_request_latch_spec s h
    simp only [attempt, h0]
    exact WP.spec_mono this (fun r ⟨a, b, c, d⟩ => ⟨Prod.ext a b, c, d⟩)
  · simp only [h0]
    have := attempt_signal_spec s h
    simp only [attempt, h0]
    exact WP.spec_mono this (fun r ⟨a, b, c, d⟩ => ⟨Prod.ext a b, c, d⟩)

/-- What `step_once` returns, in the terms of the model's `simOut`. -/
def StepPost (s : crossing.System) (r : crossing.Progress × crossing.System) : Prop :=
  match simOut (absV s) with
  | .delivered v' => r.1 = .Delivered ∧ absV r.2 = v' ∧ Inv r.2
  | .settled => r.1 = .Settled ∧ r.2 = s
  | .blocked => r.1 = .Blocked ∧ r.2 = s

theorem step_once_spec (s : crossing.System) (h : Inv s) :
    crossing.System.step_once s ⦃ r => StepPost s r ⦄ := by
  unfold crossing.System.step_once
  have hCH : crossing.System.CHILDREN.val = 2 := by simp [crossing.System.CHILDREN]
  obtain ⟨⟨l0, h0⟩, ⟨l1, h1⟩, hn⟩ := h
  have hinv : Inv s := ⟨⟨l0, h0⟩, ⟨l1, h1⟩, hn⟩
  step as ⟨child, hchild⟩
  rw [hCH] at hchild
  have hc : child.val < 2 := by omega
  step with attempt_spec s child hc hinv as ⟨a, s1, ha, hi1, hn1⟩
  have hsim : simOut (absV s) = (match attempt child.val (absV s) with
      | (.delivered, v1) => .delivered { v1 with next := (child.val + 1) % 2 }
      | (k, v1) =>
        let c1 := (v1.next + 1) % 2
        match attempt c1 v1 with
        | (.delivered, v2) => .delivered { v2 with next := (c1 + 1) % 2 }
        | (.blocked, _) => .blocked
        | (.idle, _) => match k with | .blocked => .blocked | _ => .settled) := by
    simp only [simOut, absV, hchild]
    rfl
  obtain ⟨⟨m0, hm0⟩, ⟨m1, hm1⟩, hnx⟩ := hi1
  have hi1 : Inv s1 := ⟨⟨m0, hm0⟩, ⟨m1, hm1⟩, hnx⟩
  cases a with
  | Delivered =>
    step as ⟨i, hi⟩
    step as ⟨i2, hi2⟩
    unfold StepPost
    rw [hsim, ← ha]
    simp only [attAbs]
    have hv2 : i2.val = (child.val + 1) % 2 := by rw [hi2, hCH, hi]
    refine ⟨trivial, ?_, ⟨⟨m0, hm0⟩, ⟨m1, hm1⟩, ?_⟩⟩
    · simp only [absV]; congr 1
    · show i2.val < 2
      omega
  | Blocked =>
    have hs1 : s1 = s := hn1 (by simp [attAbs])
    step as ⟨i, hi⟩
    step as ⟨child1, hchild1⟩
    rw [hCH] at hchild1
    have hc1 : child1.val < 2 := by omega
    step with attempt_spec s1 child1 hc1 hi1 as ⟨a1, s2, ha1, hi2, hn2⟩
    obtain ⟨⟨n0, hn0⟩, ⟨n1, hn1'⟩, hnx2⟩ := hi2
    have hi2 : Inv s2 := ⟨⟨n0, hn0⟩, ⟨n1, hn1'⟩, hnx2⟩
    unfold StepPost
    rw [hsim, ← ha]
    simp only [attAbs]
    have hc1eq : ((absV s1).next + 1) % 2 = child1.val := by
      rw [hchild1]; simp only [absV]; omega
    rw [hc1eq, ← ha1]
    cases a1 with
    | Delivered =>
      step as ⟨i3, hi3⟩
      step as ⟨i4, hi4⟩
      simp only [attAbs]
      have hv4 : i4.val = (child1.val + 1) % 2 := by rw [hi4, hCH, hi3]
      refine ⟨?_, ⟨⟨n0, hn0⟩, ⟨n1, hn1'⟩, ?_⟩⟩
      · simp only [absV]; congr 1
      · show i4.val < 2
        omega
    | Blocked =>
      have hs2 : s2 = s1 := hn2 (by simp [attAbs])
      simp only [attAbs, WP.spec_ok]
      simp [hs2, hs1]
    | Idle =>
      have hs2 : s2 = s1 := hn2 (by simp [attAbs])
      simp only [attAbs, WP.spec_ok]
      simp [hs2, hs1]
  | Idle =>
    have hs1 : s1 = s := hn1 (by simp [attAbs])
    step as ⟨i, hi⟩
    step as ⟨child1, hchild1⟩
    rw [hCH] at hchild1
    have hc1 : child1.val < 2 := by omega
    step with attempt_spec s1 child1 hc1 hi1 as ⟨a1, s2, ha1, hi2, hn2⟩
    obtain ⟨⟨n0, hn0⟩, ⟨n1, hn1'⟩, hnx2⟩ := hi2
    have hi2 : Inv s2 := ⟨⟨n0, hn0⟩, ⟨n1, hn1'⟩, hnx2⟩
    unfold StepPost
    rw [hsim, ← ha]
    simp only [attAbs]
    have hc1eq : ((absV s1).next + 1) % 2 = child1.val := by
      rw [hchild1]; simp only [absV]; omega
    rw [hc1eq, ← ha1]
    cases a1 with
    | Delivered =>
      step as ⟨i3, hi3⟩
      step as ⟨i4, hi4⟩
      simp only [attAbs]
      have hv4 : i4.val = (child1.val + 1) % 2 := by rw [hi4, hCH, hi3]
      refine ⟨?_, ⟨⟨n0, hn0⟩, ⟨n1, hn1'⟩, ?_⟩⟩
      · simp only [absV]; congr 1
      · show i4.val < 2
        omega
    | Blocked =>
      have hs2 : s2 = s1 := hn2 (by simp [attAbs])
      simp only [attAbs, WP.spec_ok]
      simp [hs2, hs1]
    | Idle =>
      have hs2 : s2 = s1 := hn2 (by simp [attAbs])
      simp only [attAbs, WP.spec_ok]
      simp [hs2, hs1]


/-! ### Ports and construction -/

/-- What `send_button` returns, in the terms of the model's `simSend0`. -/
def SendPost0 (s : crossing.System) (r : core.result.Result Unit crossing.Rejected × crossing.System) : Prop :=
  match simSend0 (absV s) with
  | some v' => r.1 = core.result.Result.Ok () ∧ absV r.2 = v' ∧ Inv r.2
  | none => r.1 = core.result.Result.Err () ∧ r.2 = s

def SendPost1 (s : crossing.System) (r : core.result.Result Unit crossing.Rejected × crossing.System) : Prop :=
  match simSend1 (absV s) with
  | some v' => r.1 = core.result.Result.Ok () ∧ absV r.2 = v' ∧ Inv r.2
  | none => r.1 = core.result.Result.Err () ∧ r.2 = s

theorem send_button_spec (s : crossing.System) (m : crossing.ButtonEvent) (h : Inv s) :
    crossing.System.send_button s m ⦃ r => SendPost0 s r ⦄ := by
  obtain ⟨⟨l0, h0⟩, ⟨l1, h1⟩, hn⟩ := h
  unfold crossing.System.send_button
  step with (show crossing.routes.port_button m ⦃ r => r = crossing.request_latch.Input.ButtonPressed ⦄ by
    cases m; simp [crossing.routes.port_button]) as ⟨input, hin⟩
  step with latchbox_push_spec input h0 as ⟨b, rli, hb, hr, hfull⟩
  have hlen : (absL s.request_latch_inbox).length = l0.length := by rw [absL_of_rep h0]; simp
  unfold SendPost0 simSend0
  by_cases hl : l0.length < 2
  · simp only [hl, decide_true] at hb
    simp only [hb, ite_true, WP.spec_ok]
    simp only [hl, ite_true] at hr
    simp only [absV, hlen, hl, ite_true]
    refine ⟨trivial, ?_, ⟨⟨_, hr⟩, ⟨l1, h1⟩, hn⟩⟩
    simp [absL_of_rep hr, absL_of_rep h0, hin, latchInput]
  · simp only [hl, decide_false] at hb
    simp only [hb, ite_false, WP.spec_ok, Bool.false_eq_true]
    have hs : rli = s.request_latch_inbox := hfull (by omega)
    simp only [absV, hlen, hl, ite_false]
    subst hs
    simp

theorem send_timer_spec (s : crossing.System) (m : crossing.TimerEvent) (h : Inv s) :
    crossing.System.send_timer s m ⦃ r => SendPost1 s r ⦄ := by
  obtain ⟨⟨l0, h0⟩, ⟨l1, h1⟩, hn⟩ := h
  unfold crossing.System.send_timer
  step with (show crossing.routes.port_timer m ⦃ r => r = crossing.signal.Input.TimerExpired ⦄ by
    cases m; simp [crossing.routes.port_timer]) as ⟨input, hin⟩
  step with signalbox_push_spec input h1 as ⟨b, rli, hb, hr, hfull⟩
  have hlen : (absS s.signal_inbox).length = l1.length := by rw [absS_of_rep h1]; simp
  unfold SendPost1 simSend1
  by_cases hl : l1.length < 2
  · simp only [hl, decide_true] at hb
    simp only [hb, ite_true, WP.spec_ok]
    simp only [hl, ite_true] at hr
    simp only [absV, hlen, hl, ite_true]
    refine ⟨trivial, ?_, ⟨⟨l0, h0⟩, ⟨_, hr⟩, hn⟩⟩
    simp [absS_of_rep hr, absS_of_rep h1, hin, signalInput]
  · simp only [hl, decide_false] at hb
    simp only [hb, ite_false, WP.spec_ok, Bool.false_eq_true]
    have hs : rli = s.signal_inbox := hfull (by omega)
    simp only [absV, hlen, hl, ite_false]
    subst hs
    simp

theorem new_spec : crossing.System.new ⦃ s => Inv s ∧ absV s = ⟨CrossingV1.Latch.init, CrossingV1.Signal.init, [], [], 0⟩ ⦄ := by
  unfold crossing.System.new
  step with (show crossing.request_latch.init ⦃ r => r = .Idle ⦄ by
    simp [crossing.request_latch.init, crossing.request_latch.imp.init]) as ⟨a, ha⟩
  step with latchbox_new_spec as ⟨rli, hr⟩
  step with (show crossing.signal.init ⦃ r => r = .VehiclesGreen ⦄ by
    simp [crossing.signal.init, crossing.signal.imp.init]) as ⟨b, hb⟩
  step with signalbox_new_spec as ⟨si, hs⟩
  subst ha hb
  refine ⟨⟨⟨_, hr⟩, ⟨_, hs⟩, by simp⟩, ?_⟩
  simp [absV, absL_of_rep hr, absS_of_rep hs, latchState, signalState, CrossingV1.Latch.init, CrossingV1.Signal.init]

/-! ## The refinement theorems -/

/-- The abstraction function: a Rust `System` as the Lean scheduler state of the derived network. -/
def abs (s : crossing.System) : RoundRobin.Sched N := toSched (absV s)

def buttonMsg : crossing.ButtonEvent → N.Msg ⟨0, by decide⟩
  | .Pressed => CrossingV1.Ports.ButtonEvent.pressed

def timerMsg : crossing.TimerEvent → N.Msg ⟨1, by decide⟩
  | .Expired => CrossingV1.Ports.TimerEvent.expired

/-- `System::new` cannot panic, yields a well-formed system, and abstracts to `RoundRobin.start`. -/
theorem new_refines : ∃ s, crossing.System.new = ok s ∧ Inv s ∧ abs s = RoundRobin.start N := by
  obtain ⟨s, hs, hinv, habs⟩ := spec_ok_exists new_spec
  exact ⟨s, hs, hinv, by simp only [abs, habs]; exact toSched_start⟩

/-- **`System::step_once` refines `RoundRobin.stepOnce`.** From every well-formed state it returns
without panicking; it reports `Delivered` with exactly the successor state of the model iff the model
delivers, and `Settled` / `Blocked` (leaving the system unchanged) iff the model does. -/
theorem step_once_refines (s : crossing.System) (h : Inv s) :
    ∃ r, crossing.System.step_once s = ok r ∧
      match RoundRobin.stepOnce N (abs s) with
      | .delivered s' => r.1 = crossing.Progress.Delivered ∧ abs r.2 = s' ∧ Inv r.2
      | .settled => r.1 = crossing.Progress.Settled ∧ r.2 = s
      | .blocked => r.1 = crossing.Progress.Blocked ∧ r.2 = s := by
  obtain ⟨r, hr, hpost⟩ := spec_ok_exists (step_once_spec s h)
  refine ⟨r, hr, ?_⟩
  have hl := leanOut_eq (absV s) (inv_wf h)
  unfold StepPost at hpost
  unfold leanOut at hl
  rw [← hl] at hpost
  have habs : abs s = toSched (absV s) := rfl
  rw [habs]
  cases hx : RoundRobin.stepOnce N (toSched (absV s)) with
  | delivered s' =>
    rw [hx] at hpost
    obtain ⟨h1, h2, h3⟩ := hpost
    refine ⟨h1, ?_, h3⟩
    rw [show abs r.2 = toSched (absV r.2) from rfl, h2, toSched_ofSched]
  | settled => rw [hx] at hpost; exact hpost
  | blocked => rw [hx] at hpost; exact hpost

/-- **`System::send_button` refines `RoundRobin.send` on port `button`.** `Ok(())` with exactly the
model's successor iff the model accepts; `Err(Rejected)` with the system unchanged iff it rejects. -/
theorem send_button_refines (s : crossing.System) (m : crossing.ButtonEvent) (h : Inv s) :
    ∃ r, crossing.System.send_button s m = ok r ∧
      match RoundRobin.send N (abs s) ⟨0, by decide⟩ (buttonMsg m) with
      | some s' => r.1 = core.result.Result.Ok () ∧ abs r.2 = s' ∧ Inv r.2
      | none => r.1 = core.result.Result.Err () ∧ r.2 = s := by
  obtain ⟨r, hr, hpost⟩ := spec_ok_exists (send_button_spec s m h)
  refine ⟨r, hr, ?_⟩
  have hl := leanSend0_eq (absV s) (inv_wf h)
  unfold SendPost0 at hpost
  unfold leanSend0 at hl
  rw [← hl] at hpost
  have hm : buttonMsg m = CrossingV1.Ports.ButtonEvent.pressed := by cases m; rfl
  have habs : abs s = toSched (absV s) := rfl
  rw [habs, hm]
  cases hx : RoundRobin.send N (toSched (absV s)) ⟨0, by decide⟩ (CrossingV1.Ports.ButtonEvent.pressed : N.Msg ⟨0, by decide⟩) with
  | some s' =>
    rw [hx] at hpost
    obtain ⟨h1, h2, h3⟩ := hpost
    refine ⟨h1, ?_, h3⟩
    rw [show abs r.2 = toSched (absV r.2) from rfl, h2, toSched_ofSched]
  | none => rw [hx] at hpost; exact hpost

theorem send_timer_refines (s : crossing.System) (m : crossing.TimerEvent) (h : Inv s) :
    ∃ r, crossing.System.send_timer s m = ok r ∧
      match RoundRobin.send N (abs s) ⟨1, by decide⟩ (timerMsg m) with
      | some s' => r.1 = core.result.Result.Ok () ∧ abs r.2 = s' ∧ Inv r.2
      | none => r.1 = core.result.Result.Err () ∧ r.2 = s := by
  obtain ⟨r, hr, hpost⟩ := spec_ok_exists (send_timer_spec s m h)
  refine ⟨r, hr, ?_⟩
  have hl := leanSend1_eq (absV s) (inv_wf h)
  unfold SendPost1 at hpost
  unfold leanSend1 at hl
  rw [← hl] at hpost
  have hm : timerMsg m = CrossingV1.Ports.TimerEvent.expired := by cases m; rfl
  have habs : abs s = toSched (absV s) := rfl
  rw [habs, hm]
  cases hx : RoundRobin.send N (toSched (absV s)) ⟨1, by decide⟩ (CrossingV1.Ports.TimerEvent.expired : N.Msg ⟨1, by decide⟩) with
  | some s' =>
    rw [hx] at hpost
    obtain ⟨h1, h2, h3⟩ := hpost
    refine ⟨h1, ?_, h3⟩
    rw [show abs r.2 = toSched (absV r.2) from rfl, h2, toSched_ofSched]
  | none => rw [hx] at hpost; exact hpost

/-! ## Consequence: the generated Rust inherits the model's run theorems -/

/-- The systems reachable by calling the generated API: `new`, then any mix of `step_once` and port sends
(accepted or rejected). -/
inductive RustRun : crossing.System → Prop
  | new {s} : crossing.System.new = ok s → RustRun s
  | step {s r} : RustRun s → crossing.System.step_once s = ok r → RustRun r.2
  | button {s r} (m : crossing.ButtonEvent) : RustRun s → crossing.System.send_button s m = ok r → RustRun r.2
  | timer {s r} (m : crossing.TimerEvent) : RustRun s → crossing.System.send_timer s m = ok r → RustRun r.2

/-- Every reachable system satisfies the invariant. -/
theorem rustRun_inv {s} (h : RustRun s) : Inv s := by
  induction h with
  | new hs =>
    obtain ⟨s', hs', hinv, _⟩ := new_refines
    obtain rfl := Result.ok_injective (hs.symm.trans hs')
    exact hinv
  | step _ hr ih =>
    obtain ⟨r', hr', hp⟩ := step_once_refines _ ih
    obtain rfl := Result.ok_injective (hr.symm.trans hr')
    revert hp
    cases hx : RoundRobin.stepOnce N (abs _) with
    | delivered s' => intro hp; exact hp.2.2
    | settled => intro hp; rw [hp.2]; exact ih
    | blocked => intro hp; rw [hp.2]; exact ih
  | button m _ hr ih =>
    obtain ⟨r', hr', hp⟩ := send_button_refines _ m ih
    obtain rfl := Result.ok_injective (hr.symm.trans hr')
    revert hp
    cases hx : RoundRobin.send N (abs _) _ (buttonMsg m) with
    | some s' => intro hp; exact hp.2.2
    | none => intro hp; rw [hp.2]; exact ih
  | timer m _ hr ih =>
    obtain ⟨r', hr', hp⟩ := send_timer_refines _ m ih
    obtain rfl := Result.ok_injective (hr.symm.trans hr')
    revert hp
    cases hx : RoundRobin.send N (abs _) _ (timerMsg m) with
    | some s' => intro hp; exact hp.2.2
    | none => intro hp; rw [hp.2]; exact ih

/-- **Every system reachable through the generated API abstracts to a run of the Lean scheduler model**
under the most permissive environment. Hence everything proved about `RoundRobin.Run` (for example
`run_bounded`) holds of the generated Rust. -/
theorem rustRun_simulates {s} (h : RustRun s) : RoundRobin.Run N (anyEnv N) (abs s) := by
  induction h with
  | new hs =>
    obtain ⟨s', hs', _, habs⟩ := new_refines
    obtain rfl := Result.ok_injective (hs.symm.trans hs')
    rw [habs]; exact RoundRobin.Run.start
  | step hs hr ih =>
    have hinv := rustRun_inv hs
    obtain ⟨r', hr', hp⟩ := step_once_refines _ hinv
    obtain rfl := Result.ok_injective (hr.symm.trans hr')
    revert hp
    cases hx : RoundRobin.stepOnce N (abs _) with
    | delivered s' => intro hp; rw [hp.2.1]; exact RoundRobin.Run.step ih hx
    | settled => intro hp; rw [hp.2]; exact ih
    | blocked => intro hp; rw [hp.2]; exact ih
  | button m hs hr ih =>
    have hinv := rustRun_inv hs
    obtain ⟨r', hr', hp⟩ := send_button_refines _ m hinv
    obtain rfl := Result.ok_injective (hr.symm.trans hr')
    revert hp
    cases hx : RoundRobin.send N (abs _) _ (buttonMsg m) with
    | some s' => intro hp; rw [hp.2.1]; exact RoundRobin.Run.send _ (buttonMsg m) ih trivial hx
    | none => intro hp; rw [hp.2]; exact ih
  | timer m hs hr ih =>
    have hinv := rustRun_inv hs
    obtain ⟨r', hr', hp⟩ := send_timer_refines _ m hinv
    obtain rfl := Result.ok_injective (hr.symm.trans hr')
    revert hp
    cases hx : RoundRobin.send N (abs _) _ (timerMsg m) with
    | some s' => intro hp; rw [hp.2.1]; exact RoundRobin.Run.send _ (timerMsg m) ih trivial hx
    | none => intro hp; rw [hp.2]; exact ih

/-- The generated Rust never overflows a mailbox (the model's `run_bounded`, transferred). -/
theorem rustRun_bounded {s} (h : RustRun s) : Bounded N (abs s).cfg :=
  RoundRobin.run_bounded N (anyEnv N) (rustRun_simulates h)

end AeneasV1.Refinement
