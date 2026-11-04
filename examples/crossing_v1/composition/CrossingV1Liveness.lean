import CrossingV1Properties
open L2R.Async1
open CrossingV1
open CrossingV1Properties

/-!
# Liveness of the repaired crossing, under stated assumptions (`composite/v1`)

**What is proved.** Over every trace of the derived network (semantics
`l2r.async/v1`) that starts from a reachable configuration, in any environment `E`:

> if deliveries are *weakly fair* and timer expiries are *accepted again and again*,
> then every pending request is acknowledged (the latch becomes idle).

This is `pending_request_acknowledged`. It instantiates the generic
`L2R.Async1.fair_termination` with an explicit ranking `mu` and helpful action `hlp`.
The ranking conditions are checked on the finite abstraction of
`CrossingV1Properties` by kernel `decide` (no `native_decide`), for **this example at
its stated capacities (2, 2)** only; they are not a generic result.

**The hypotheses, and what each is not.**

* `WeaklyFair` (scheduling): a delivery that stays enabled from some time on is
  taken. It is a hypothesis on the trace, not a consequence of the semantics. It is
  weaker than *strong* fairness (enabled infinitely often ⇒ taken infinitely often),
  which is **not** assumed here.
* `Recurs IsTimerSend` (environmental progress): timer expiries are **accepted**
  infinitely often. "Accepted" matters. With bounded mailboxes a send is accepted
  only if the target has room, so acceptance is partly an outcome of the system, not
  a pure property of the environment. A timer that *fires* periodically but is always
  *rejected* does not satisfy it, and for such runs the conclusion can fail even
  under weak fairness: see `CrossingV1LivenessWitness.starvation_possible`.
* No assumption restricts the environment `E` or the order of sends.

**What the theorem does not say.** It says nothing about executions that stop
(finite executions are not traces). A request can be outstanding in a quiescent
system that only the environment can move (`CrossingV1LivenessWitness.quiescent_pending`).
That the hypotheses are satisfiable, and that each is needed, is proved in
`CrossingV1LivenessWitness`.
-/

namespace CrossingV1Liveness

inductive ALab | dL | dG | press | tick deriving DecidableEq, Repr

def cpress : CCfg → List CCfg
  | (ls, ss, li, si) => if li.length < 2 then [(ls, ss, li ++ [Latch.Input.buttonPressed], si)] else []
def ctick : CCfg → List CCfg
  | (ls, ss, li, si) => if si.length < 2 then [(ls, ss, li, si ++ [Signal.Input.timerExpired])] else []

def ledges (c : CCfg) : List (ALab × CCfg) :=
  (cdeliverL c).toList.map (fun d => (ALab.dL, d)) ++ (cdeliverG c).toList.map (fun d => (ALab.dG, d)) ++
  (cpress c).map (fun d => (ALab.press, d)) ++ (ctick c).map (fun d => (ALab.tick, d))

def isReq : Signal.Input → Bool | .crossingRequested => true | _ => false
def isAck : Latch.Input → Bool | .acknowledged => true | _ => false
def held : Signal.State → Bool | .vehiclesAmber | .walkQueued | .clearanceQueued => true | _ => false
def need : Signal.State → Nat | .vehiclesAmber => 1 | .clearanceQueued => 2 | .walkQueued => 3 | _ => 0
def pend (c : CCfg) : Bool := c.1 == Latch.State.pending

def mu (c : CCfg) : Nat :=
  if c.2.2.2.any isReq then 20 + c.2.2.2.findIdx isReq
  else if held c.2.1 then 3 * need c.2.1 + (2 - c.2.2.2.length)
  else c.2.2.1.findIdx isAck

def hlp (c : CCfg) : Option (Fin N.size) :=
  if c.2.2.2.any isReq then some ⟨1, by decide⟩ else if held c.2.1 then none else some ⟨0, by decide⟩

def decr (c c' : CCfg) : Bool := !pend c' || decide (mu c' < mu c)
def enabledA (d : Fin N.size) (c : CCfg) : Bool := if d.val = 0 then (cdeliverL c).isSome else (cdeliverG c).isSome
def labOf (d : Fin N.size) : ALab := if d.val = 0 then .dL else .dG


/-- Both children. -/
def kids : List (Fin N.size) := [⟨0, by decide⟩, ⟨1, by decide⟩]

theorem mem_kids (d : Fin N.size) : d ∈ kids := by
  rcases child_cases d with rfl | rfl <;> decide

/-- The ranking conditions of `fair_termination`, stated on the finite abstraction. -/
def LiveCond (c : CCfg) : Prop :=
  pend c = true →
    (∀ d ∈ kids, hlp c = some d → enabledA d c = true) ∧
    (∀ d ∈ kids, hlp c = some d → ∀ e ∈ ledges c, e.1 = labOf d → (pend e.2 = false ∨ mu e.2 < mu c)) ∧
    (∀ d ∈ kids, hlp c = some d → ∀ e ∈ ledges c, e.1 ≠ labOf d →
      (pend e.2 = false ∨ mu e.2 < mu c ∨ (mu e.2 = mu c ∧ hlp e.2 = some d ∧ enabledA d e.2 = true))) ∧
    (hlp c = none → ∀ e ∈ ledges c, e.1 ≠ ALab.tick →
      (pend e.2 = false ∨ mu e.2 < mu c ∨ (mu e.2 = mu c ∧ hlp e.2 = none))) ∧
    (hlp c = none → ∀ e ∈ ledges c, e.1 = ALab.tick → (pend e.2 = false ∨ mu e.2 < mu c))

set_option synthInstance.maxSize 4096 in
instance : DecidablePred LiveCond := fun c => by unfold LiveCond; infer_instance

theorem live_cond : ∀ c ∈ reach, LiveCond c := by decide +kernel

/-! ## From permitted steps to labelled abstract edges -/

def alab : Label N → ALab
  | .deliver c => if c.val = 0 then .dL else .dG
  | .send p _ => if p.val = 0 then .press else .tick

theorem alab_eq_labOf (l : Label N) (d : Fin N.size) : alab l = labOf d ↔ l = .deliver d := by
  have hd : d.val < 2 := d.isLt
  cases l with
  | deliver c =>
    have hc : c.val < 2 := c.isLt
    simp only [alab, labOf, Label.deliver.injEq]
    constructor
    · intro h
      apply Fin.ext
      by_cases h1 : c.val = 0 <;> by_cases h2 : d.val = 0 <;> simp_all <;> omega
    · intro h; subst h; rfl
  | send p m =>
    simp only [alab, labOf]
    constructor
    · intro h; by_cases h1 : p.val = 0 <;> by_cases h2 : d.val = 0 <;> simp_all
    · intro h; cases h

theorem step_lab {E : Env N} {k k' : Config N} {l : Label N} (h : StepL N E k l k') :
    (alab l, view k') ∈ ledges (view k) := by
  cases h with
  | deliver c hd =>
    rcases child_cases c with rfl | rfl
    · have := sim_deliver_latch hd
      simp [ledges, alab, this]
    · have := sim_deliver_signal hd
      simp [ledges, alab, this]
  | send p m _ hroom =>
    rcases port_cases p with rfl | rfl
    · cases m
      have hr : (latchIn k).length < 2 := hroom
      rw [view_send_button]
      simp [ledges, alab, cpress, view, hr]
    · cases m
      have hr : (signalIn k).length < 2 := hroom
      rw [view_send_timer]
      simp [ledges, alab, ctick, view, hr]

theorem bounded_of_reach {k : Config N} (h : view k ∈ reach) : Bounded N k := by
  intro c
  obtain ⟨h1, h2⟩ := reach_bounded _ h
  rcases child_cases c with rfl | rfl
  · exact h1
  · exact h2

theorem enabled_real {k : Config N} (hr : view k ∈ reach) (d : Fin N.size)
    (h : enabledA d (view k) = true) : (deliver N k d).isSome = true := by
  have hb := bounded_of_reach hr
  rcases child_cases d with rfl | rfl
  · exact enabled_latch hb rfl (by simpa [enabledA] using h)
  · exact enabled_signal hb rfl (by simpa [enabledA] using h)

/-- The environment's progress assumption: a timer expiry is *accepted* (sent through
the timer port, with room in the signal's mailbox). -/
def IsTimerSend : Label N → Prop := fun l => alab l = .tick

theorem isTimerSend_iff (l : Label N) : IsTimerSend l ↔ ∃ m, l = .send ⟨1, by decide⟩ m := by
  cases l with
  | deliver c =>
    simp only [IsTimerSend, alab]
    constructor
    · intro h; split at h <;> simp at h
    · rintro ⟨_, h⟩; cases h
  | send p m =>
    simp only [IsTimerSend, alab]
    constructor
    · intro h
      have hp : p.val = 1 := by
        have := p.isLt
        by_cases h0 : p.val = 0
        · simp [h0] at h
        · have : p.val < 2 := p.isLt
          omega
      have : p = ⟨1, by decide⟩ := Fin.ext hp
      subst this
      exact ⟨m, rfl⟩
    · rintro ⟨m', h⟩
      injection h with h1
      subst h1
      rfl

def helper (k : Config N) : Option (Fin N.size) := hlp (view k)
def rank (k : Config N) : Nat := mu (view k)
def Pend (k : Config N) : Prop := latch k = .pending ∧ view k ∈ reach

theorem pend_iff (k : Config N) : pend (view k) = true ↔ latch k = .pending := by
  simp [pend, view]

/-- **Liveness under fairness.** On every trace of the repaired crossing that starts from a
reachable configuration and satisfies

* weak fairness of deliveries (a delivery enabled from some time on is taken), and
* environmental progress: timer expiries are accepted again and again,

a pending request is acknowledged: the latch becomes idle. -/
theorem pending_request_acknowledged {E : Env N} (tr : Trace N E)
    (h0 : Reachable N E (tr.cfg 0)) (hwf : WeaklyFair N tr)
    (htimer : Recurs N tr (IsTimerSend)) :
    ∀ t, latch (tr.cfg t) = .pending → ∃ s, t ≤ s ∧ latch (tr.cfg s) = .idle := by
  have hreach : ∀ t, Reachable N E (tr.cfg t) := by
    intro t; induction t with
    | zero => exact h0
    | succ t ih => exact Reachable.step ih ⟨_, tr.valid t⟩
  have hv : ∀ t, view (tr.cfg t) ∈ reach := fun t => view_reachable E (hreach t)
  -- the labelled abstract edge taken at time s
  have edge : ∀ s, (alab (tr.lab s), view (tr.cfg (s + 1))) ∈ ledges (view (tr.cfg s)) :=
    fun s => step_lab (tr.valid s)
  have cond : ∀ s, LiveCond (view (tr.cfg s)) := fun s => live_cond _ (hv s)
  have notP : ∀ s, ¬ Pend (tr.cfg s) ↔ ¬ latch (tr.cfg s) = .pending := by
    intro s; simp only [Pend, hv s, and_true]
  have key : ∀ t, Pend (tr.cfg t) → ∃ s, t ≤ s ∧ ¬ Pend (tr.cfg s) := by
    refine fair_termination N tr hwf IsTimerSend htimer Pend rank helper ?hp ?he ?hd ?hpe ?hde
    case hp =>
      intro s d hP hh hne
      have hpe := (pend_iff _).mpr hP.1
      obtain ⟨_, _, h3, _⟩ := cond s hpe
      have hne' : alab (tr.lab s) ≠ labOf d := fun h => hne ((alab_eq_labOf _ _).mp h)
      rcases h3 d (mem_kids d) hh _ (edge s) hne' with h | h | ⟨h1, h2, h3⟩
      · left; rw [notP]; intro hp; simp [(pend_iff _).mpr hp] at h
      · right; left; exact h
      · right; right; exact ⟨h1, h2, enabled_real (hv _) d h3⟩
    case he =>
      intro s d hP hh
      have hpe := (pend_iff _).mpr hP.1
      obtain ⟨h1, _⟩ := cond s hpe
      exact enabled_real (hv s) d (h1 d (mem_kids d) hh)
    case hd =>
      intro s d hP hh hl
      have hpe := (pend_iff _).mpr hP.1
      obtain ⟨_, h2, _⟩ := cond s hpe
      have : alab (tr.lab s) = labOf d := (alab_eq_labOf _ _).mpr hl
      rcases h2 d (mem_kids d) hh _ (edge s) this with h | h
      · left; rw [notP]; intro hp; simp [(pend_iff _).mpr hp] at h
      · right; exact h
    case hpe =>
      intro s hP hh hq
      have hpe := (pend_iff _).mpr hP.1
      obtain ⟨_, _, _, h4, _⟩ := cond s hpe
      have hne : alab (tr.lab s) ≠ ALab.tick := hq
      rcases h4 hh _ (edge s) hne with h | h | ⟨h1, h2⟩
      · left; rw [notP]; intro hp; simp [(pend_iff _).mpr hp] at h
      · right; left; exact h
      · right; right; exact ⟨h1, h2⟩
    case hde =>
      intro s hP hh hq
      have hpe := (pend_iff _).mpr hP.1
      obtain ⟨_, _, _, _, h5⟩ := cond s hpe
      have : alab (tr.lab s) = ALab.tick := hq
      rcases h5 hh _ (edge s) this with h | h
      · left; rw [notP]; intro hp; simp [(pend_iff _).mpr hp] at h
      · right; exact h


  intro t ht
  obtain ⟨s, hs, hn⟩ := key t ⟨ht, hv t⟩
  refine ⟨s, hs, ?_⟩
  rw [notP] at hn
  cases hl : latch (tr.cfg s) with
  | idle => rfl
  | pending => exact absurd hl hn

end CrossingV1Liveness
