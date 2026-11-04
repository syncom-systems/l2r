import CrossingV1Liveness

/-!
# Are the liveness hypotheses satisfiable, and is each needed?

`CrossingV1Liveness.pending_request_acknowledged` is conditional. This file proves,
about the same derived network, that

1. **the hypotheses are satisfiable**, and not vacuously: some trace is weakly
   fair, accepts timer expiries again and again, starts reachable, and has a pending
   request (`hypotheses_satisfiable`);
2. **weak fairness is needed**: with a timer that keeps being accepted, an
   acknowledgement can wait forever in the latch's mailbox (`fairness_needed`);
3. **accepted timer progress is needed, and a periodic timer is not enough**: under
   weak fairness a pending request can wait forever while the timer is offered all
   the time but never accepted, because a flood of presses keeps the latch's mailbox
   full (`starvation_possible`). Weak fairness does not prevent this; *strong*
   fairness of deliveries would, and a round-robin scheduler does not provide it;
4. **finite executions are not silently excluded**: a reachable configuration with a
   pending request and nothing left to deliver exists (`quiescent_pending`), where
   only the environment can move, and every reachable configuration has a successor
   (`no_dead_end`).

All traces below are built from finite cycles of the abstraction, realised as
real permitted steps (`ledges_realized`).
-/

open L2R.Async1
open L2R.Async (update update_same update_other)
open CrossingV1
open CrossingV1Properties
open CrossingV1Liveness

namespace CrossingV1LivenessWitness

/-! ## Realising abstract edges -/

/-- The configuration with the given machine states and mailboxes. -/
def ofView (c : CCfg) : Config N :=
  ⟨fun i => match i with
      | ⟨0, _⟩ => c.1
      | ⟨1, _⟩ => c.2.1
      | ⟨n + 2, h⟩ => absurd h (by have : N.size = 2 := rfl; omega),
   fun i => match i with
      | ⟨0, _⟩ => c.2.2.1
      | ⟨1, _⟩ => c.2.2.2
      | ⟨n + 2, h⟩ => absurd h (by have : N.size = 2 := rfl; omega)⟩

theorem view_ofView (c : CCfg) : view (ofView c) = c := rfl

theorem ofView_view (k : Config N) : ofView (view k) = k := by
  cases k with
  | mk st ib =>
    simp only [ofView, view, latch, signal, latchIn, signalIn]
    congr 1
    · funext i
      rcases child_cases i with rfl | rfl <;> rfl
    · funext i
      rcases child_cases i with rfl | rfl <;> rfl

theorem ofView_initial : ofView initialC = initial N := by
  rw [← view_initial, ofView_view]

/-- Every abstract edge from a bounded configuration is a permitted step with that label. -/
theorem ledges_realized {k : Config N} (hb : Bounded N k) {a : ALab} {d : CCfg}
    (hd : (a, d) ∈ ledges (view k)) :
    ∃ l k', StepL N (anyEnv N) k l k' ∧ alab l = a ∧ view k' = d := by
  simp only [ledges, List.mem_append, List.mem_map, Option.mem_toList] at hd
  rcases hd with ((⟨e, he, h⟩ | ⟨e, he, h⟩) | ⟨e, he, h⟩) | ⟨e, he, h⟩
  · obtain ⟨rfl, rfl⟩ := Prod.mk.inj h
    have hs : (cdeliverL (view k)).isSome := by simp [he]
    obtain ⟨k', hk'⟩ := Option.isSome_iff_exists.mp (enabled_latch hb rfl hs)
    have := sim_deliver_latch hk'
    rw [he] at this
    exact ⟨.deliver L, k', StepL.deliver (N := N) (E := anyEnv N) L hk', rfl, (Option.some.inj this).symm⟩
  · obtain ⟨rfl, rfl⟩ := Prod.mk.inj h
    have hs : (cdeliverG (view k)).isSome := by simp [he]
    obtain ⟨k', hk'⟩ := Option.isSome_iff_exists.mp (enabled_signal hb rfl hs)
    have := sim_deliver_signal hk'
    rw [he] at this
    exact ⟨.deliver G, k', StepL.deliver (N := N) (E := anyEnv N) G hk', rfl, (Option.some.inj this).symm⟩
  · obtain ⟨rfl, rfl⟩ := Prod.mk.inj h
    simp only [cpress, view] at he
    by_cases hr : (latchIn k).length < 2
    · simp only [hr, ite_true, List.mem_singleton] at he
      refine ⟨.send ⟨0, by decide⟩ .pressed, inject N k ⟨0, by decide⟩ .pressed,
        StepL.send k ⟨0, by decide⟩ .pressed trivial hr, rfl, ?_⟩
      rw [view_send_button, he]
    · simp [hr] at he
  · obtain ⟨rfl, rfl⟩ := Prod.mk.inj h
    simp only [ctick, view] at he
    by_cases hr : (signalIn k).length < 2
    · simp only [hr, ite_true, List.mem_singleton] at he
      refine ⟨.send ⟨1, by decide⟩ .expired, inject N k ⟨1, by decide⟩ .expired,
        StepL.send k ⟨1, by decide⟩ .expired trivial hr, rfl, ?_⟩
      rw [view_send_timer, he]
    · simp [hr] at he

theorem enabled_abs {k : Config N} {d : Fin N.size} (h : (deliver N k d).isSome = true) :
    enabledA d (view k) = true := by
  obtain ⟨k', hk'⟩ := Option.isSome_iff_exists.mp h
  rcases child_cases d with rfl | rfl
  · simp [enabledA, sim_deliver_latch hk']
  · simp [enabledA, sim_deliver_signal hk']

/-- A trace following a sequence of abstract edges. -/
noncomputable def mkTrace (a : Nat → CCfg) (lb : Nat → ALab) (hr : ∀ t, a t ∈ reach)
    (he : ∀ t, (lb t, a (t + 1)) ∈ ledges (a t)) : Trace N (anyEnv N) :=
  have real : ∀ t, ∃ l, StepL N (anyEnv N) (ofView (a t)) l (ofView (a (t + 1))) ∧ alab l = lb t := by
    intro t
    have hb : Bounded N (ofView (a t)) := bounded_of_reach (by rw [view_ofView]; exact hr t)
    obtain ⟨l, k', hs, hl, hv⟩ := ledges_realized hb (a := lb t) (d := a (t + 1)) (by rw [view_ofView]; exact he t)
    have : k' = ofView (a (t + 1)) := by rw [← ofView_view k', hv]
    exact ⟨l, this ▸ hs, hl⟩
  { cfg := fun t => ofView (a t)
    lab := fun t => (real t).choose
    valid := fun t => (real t).choose_spec.1 }

theorem mkTrace_alab (a : Nat → CCfg) (lb : Nat → ALab) (hr : ∀ t, a t ∈ reach)
    (he : ∀ t, (lb t, a (t + 1)) ∈ ledges (a t)) (t : Nat) :
    alab ((mkTrace a lb hr he).lab t) = lb t := by
  unfold mkTrace
  exact (Classical.choose_spec (p := fun l => StepL N (anyEnv N) (ofView (a t)) l (ofView (a (t + 1))) ∧ alab l = lb t) _).2

theorem mkTrace_wf (a : Nat → CCfg) (lb : Nat → ALab) (hr : ∀ t, a t ∈ reach)
    (he : ∀ t, (lb t, a (t + 1)) ∈ ledges (a t))
    (h : ∀ t (d : Fin N.size), (∀ s, t ≤ s → enabledA d (a s) = true) → ∃ s, t ≤ s ∧ lb s = labOf d) :
    WeaklyFair N (mkTrace a lb hr he) := by
  intro t c hen
  obtain ⟨s, hs, hl⟩ := h t c (fun s hs => by
    have := enabled_abs (hen s hs)
    simpa [mkTrace, view_ofView] using this)
  exact ⟨s, hs, (alab_eq_labOf _ _).mp (by rw [mkTrace_alab]; exact hl)⟩

theorem mkTrace_recurs (a : Nat → CCfg) (lb : Nat → ALab) (hr : ∀ t, a t ∈ reach)
    (he : ∀ t, (lb t, a (t + 1)) ∈ ledges (a t)) (h : ∀ t, ∃ s, t ≤ s ∧ lb s = ALab.tick) :
    Recurs N (mkTrace a lb hr he) IsTimerSend := by
  intro t
  obtain ⟨s, hs, hl⟩ := h t
  exact ⟨s, hs, by simp only [IsTimerSend]; rw [mkTrace_alab]; exact hl⟩

/-- Reachability is preserved along abstract edges. -/
theorem reachable_step {c d : CCfg} {a : ALab} (hc : Reachable N (anyEnv N) (ofView c)) (hr : c ∈ reach)
    (he : (a, d) ∈ ledges c) : Reachable N (anyEnv N) (ofView d) := by
  have hb : Bounded N (ofView c) := bounded_of_reach (by rw [view_ofView]; exact hr)
  obtain ⟨l, k', hs, _, hv⟩ := ledges_realized hb (a := a) (d := d) (by rw [view_ofView]; exact he)
  have : k' = ofView d := by rw [← ofView_view k', hv]
  exact this ▸ Reachable.step hc ⟨l, hs⟩

theorem reachable_initial : Reachable N (anyEnv N) (ofView initialC) := by
  rw [ofView_initial]; exact Reachable.init

/-! ## Periodic sequences -/

theorem periodic_recurs (P : Nat) (hP : 0 < P) (lbs : Nat → ALab) (hper : ∀ s, lbs s = lbs (s % P)) (x : ALab)
    (h : ∃ r, r < P ∧ lbs r = x) (t : Nat) : ∃ s, t ≤ s ∧ lbs s = x := by
  obtain ⟨r, hr, hx⟩ := h
  refine ⟨t + (r + P - t % P) % P, by omega, ?_⟩
  have : (t + (r + P - t % P) % P) % P = r := by
    have h1 : t % P < P := Nat.mod_lt _ hP
    have h2 : t = P * (t / P) + t % P := (Nat.div_add_mod t P).symm
    generalize t % P = m at *
    generalize t / P = q at *
    subst h2
    have : (P * q + m + (r + P - m) % P) % P = r := by
      rcases Nat.lt_or_ge r m with h3 | h3
      · have : (r + P - m) % P = r + P - m := Nat.mod_eq_of_lt (by omega)
        rw [this]
        have : P * q + m + (r + P - m) = P * (q + 1) + r := by
          rw [Nat.mul_add]; omega
        rw [this, Nat.mul_add_mod, Nat.mod_eq_of_lt hr]
      · have : (r + P - m) % P = r - m := by
          have : r + P - m = (r - m) + P := by omega
          rw [this, Nat.add_mod_right, Nat.mod_eq_of_lt (by omega)]
        rw [this]
        have : P * q + m + (r - m) = P * q + r := by omega
        rw [this, Nat.mul_add_mod, Nat.mod_eq_of_lt hr]
    exact this
  rw [hper, this]; exact hx


/-! ## 1. The hypotheses are satisfiable (a full request cycle)

Ten steps return to the initial configuration: a press, the request travelling to
the signal, amber, a timer expiry, the walk phase and its acknowledgement, and two
more timer expiries to return to green. The latch is pending along the way. -/

abbrev bt : Latch.Input := .buttonPressed
abbrev ak : Latch.Input := .acknowledged
abbrev rq : Signal.Input := .crossingRequested
abbrev tk : Signal.Input := .timerExpired

def cycle : List CCfg :=
  [ (.idle, .vehiclesGreen, [], []),
    (.idle, .vehiclesGreen, [bt], []),
    (.pending, .vehiclesGreen, [], [rq]),
    (.pending, .vehiclesAmber, [], []),
    (.pending, .vehiclesAmber, [], [tk]),
    (.pending, .walk, [ak], []),
    (.idle, .walk, [], []),
    (.idle, .walk, [], [tk]),
    (.idle, .clearance, [], []),
    (.idle, .clearance, [], [tk]) ]

def cycleLabels : List ALab :=
  [.press, .dL, .dG, .tick, .dG, .dL, .tick, .dG, .tick, .dG]

def cycA (r : Nat) : CCfg := cycle.getD (r % 10) initialC
def cycL (r : Nat) : ALab := cycleLabels.getD (r % 10) .dL

theorem cyc_ok' : ∀ r ∈ List.range 10, cycA r ∈ reach ∧ (cycL r, cycA (r + 1)) ∈ ledges (cycA r) := by
  decide +kernel

theorem cyc_ok : ∀ r, r < 10 → cycA r ∈ reach ∧ (cycL r, cycA (r + 1)) ∈ ledges (cycA r) :=
  fun r hr => cyc_ok' r (List.mem_range.mpr hr)

theorem cycL_per (s : Nat) : cycL s = cycL (s % 10) := by simp [cycL, Nat.mod_mod]

theorem cycA_reach (t : Nat) : cycA t ∈ reach := by
  have := (cyc_ok (t % 10) (Nat.mod_lt _ (by omega))).1
  simpa [cycA] using this

theorem cycA_edge (t : Nat) : (cycL t, cycA (t + 1)) ∈ ledges (cycA t) := by
  have := (cyc_ok (t % 10) (Nat.mod_lt _ (by omega))).2
  have h1 : (t % 10 + 1) % 10 = (t + 1) % 10 := by omega
  simpa [cycA, cycL, Nat.mod_mod, h1] using this

noncomputable abbrev satTrace : Trace N (anyEnv N) := mkTrace cycA cycL cycA_reach cycA_edge

/-- **The hypotheses of `pending_request_acknowledged` are satisfiable**, not vacuously:
a trace starting from the initial configuration, weakly fair, with timer expiries
accepted again and again, in which the latch is pending at some time. -/
theorem hypotheses_satisfiable :
    ∃ tr : Trace N (anyEnv N), Reachable N (anyEnv N) (tr.cfg 0) ∧ WeaklyFair N tr ∧
      Recurs N tr IsTimerSend ∧ ∃ t, latch (tr.cfg t) = .pending := by
  refine ⟨satTrace, reachable_initial, ?_, ?_, 2, rfl⟩
  · apply mkTrace_wf
    intro t d _
    obtain ⟨s, hs, h⟩ := periodic_recurs 10 (by omega) cycL cycL_per (labOf d) (by
      rcases child_cases d with rfl | rfl
      · exact ⟨1, by decide, rfl⟩
      · exact ⟨2, by decide, rfl⟩) t
    exact ⟨s, hs, h⟩
  · apply mkTrace_recurs
    intro t
    obtain ⟨s, hs, h⟩ := periodic_recurs 10 (by omega) cycL cycL_per .tick ⟨3, by decide, rfl⟩ t
    exact ⟨s, hs, h⟩

/-! ## Paths into the interesting configurations -/

/-- Reachable abstract configurations, listed with the edge that reaches each. -/
theorem reach_pend_amber : Reachable N (anyEnv N) (ofView (.pending, .vehiclesAmber, [], [])) := by
  have h1 := reachable_step (c := initialC) (d := (.idle, .vehiclesGreen, [bt], [])) (a := .press)
    reachable_initial (by decide +kernel) (by decide +kernel)
  have h2 := reachable_step (c := (.idle, .vehiclesGreen, [bt], [])) (d := (.pending, .vehiclesGreen, [], [rq])) (a := .dL)
    h1 (by decide +kernel) (by decide +kernel)
  exact reachable_step (c := (.pending, .vehiclesGreen, [], [rq])) (d := (.pending, .vehiclesAmber, [], [])) (a := .dG)
    h2 (by decide +kernel) (by decide +kernel)

/-! ## 2. Weak fairness is needed -/

def stuckA : CCfg := (.pending, .vehiclesAmber, [bt, bt], [tk, tk])
def stuckB : CCfg := (.pending, .vehiclesAmber, [bt], [tk, tk])

def starveA (t : Nat) : CCfg := if t % 2 = 0 then stuckA else stuckB
def starveL (t : Nat) : ALab := if t % 2 = 0 then .dL else .press

theorem starve_ok : stuckA ∈ reach ∧ stuckB ∈ reach ∧ (ALab.dL, stuckB) ∈ ledges stuckA ∧
    (ALab.press, stuckA) ∈ ledges stuckB ∧ enabledA ⟨1, by decide⟩ stuckA = false := by
  decide +kernel

theorem starveA_reach (t : Nat) : starveA t ∈ reach := by
  unfold starveA; split
  · exact starve_ok.1
  · exact starve_ok.2.1

theorem starveA_edge (t : Nat) : (starveL t, starveA (t + 1)) ∈ ledges (starveA t) := by
  unfold starveA starveL
  by_cases h : t % 2 = 0
  · have h' : ¬ (t + 1) % 2 = 0 := by omega
    simp [h, h', starve_ok.2.2.1]
  · have h' : (t + 1) % 2 = 0 := by omega
    simp [h, h', starve_ok.2.2.2.1]

theorem reach_stuck : Reachable N (anyEnv N) (ofView stuckA) := by
  have h1 := reachable_step (c := (.pending, .vehiclesAmber, [], [])) (d := (.pending, .vehiclesAmber, [bt], [])) (a := .press)
    reach_pend_amber (by decide +kernel) (by decide +kernel)
  have h2 := reachable_step (c := (.pending, .vehiclesAmber, [bt], [])) (d := (.pending, .vehiclesAmber, [bt, bt], [])) (a := .press)
    h1 (by decide +kernel) (by decide +kernel)
  have h3 := reachable_step (c := (.pending, .vehiclesAmber, [bt, bt], [])) (d := (.pending, .vehiclesAmber, [bt, bt], [tk])) (a := .tick)
    h2 (by decide +kernel) (by decide +kernel)
  exact reachable_step (c := (.pending, .vehiclesAmber, [bt, bt], [tk])) (d := stuckA) (a := .tick)
    h3 (by decide +kernel) (by decide +kernel)

noncomputable abbrev starveTrace : Trace N (anyEnv N) := mkTrace starveA starveL starveA_reach starveA_edge

/-- **Weak fairness does not give liveness here; accepted timer progress is needed.**
There is a weakly fair trace, from a reachable configuration, in which the latch is
pending at every time and **no timer expiry is ever accepted**: the signal's
mailbox is full, and a stream of presses keeps the latch's mailbox full whenever the
signal could deliver. The signal's delivery is enabled only intermittently, so weak
fairness (which constrains only continuously enabled deliveries) says nothing about
it. A timer that merely *fires* periodically does not help: its sends are rejected.
(*Strong* fairness would exclude this run; a round-robin scheduler does not provide
it, because the caller's press lands between two scheduler calls.) -/
theorem starvation_possible :
    ∃ tr : Trace N (anyEnv N), Reachable N (anyEnv N) (tr.cfg 0) ∧ WeaklyFair N tr ∧
      ∀ t, latch (tr.cfg t) = .pending ∧ ¬ IsTimerSend (tr.lab t) := by
  refine ⟨starveTrace, reach_stuck, ?_, ?_⟩
  · intro t c hen
    rcases child_cases c with rfl | rfl
    · refine ⟨t + t % 2, by omega, ?_⟩
      apply (alab_eq_labOf _ _).mp
      rw [mkTrace_alab (a := starveA) (lb := starveL)]
      have : (t + t % 2) % 2 = 0 := by omega
      simp [starveL, this, labOf]
    · exfalso
      have h := enabled_abs (hen (t + t % 2) (by omega))
      have : (t + t % 2) % 2 = 0 := by omega
      have h2 : enabledA ⟨1, by decide⟩ (starveA (t + t % 2)) = true := by
        simpa [starveTrace, mkTrace, view_ofView] using h
      simp [starveA, this] at h2
      have h3 : enabledA ⟨1, by decide⟩ stuckA = true := h2
      rw [starve_ok.2.2.2.2] at h3
      cases h3
  · intro t
    refine ⟨?_, ?_⟩
    · show latch (ofView (starveA t)) = _
      unfold starveA; split <;> rfl
    · intro h
      simp only [IsTimerSend] at h
      rw [mkTrace_alab (a := starveA) (lb := starveL)] at h
      unfold starveL at h; split at h <;> cases h

/-! ## 3. Environmental progress is needed: weak fairness without it is not enough, and
fairness without weak fairness's counterpart is not enough either -/

def waitA : CCfg := (.pending, .vehiclesGreen, [ak], [])
def waitB : CCfg := (.pending, .vehiclesGreen, [ak], [tk])

def waitCfg (t : Nat) : CCfg := if t % 2 = 0 then waitA else waitB
def waitL (t : Nat) : ALab := if t % 2 = 0 then .tick else .dG

theorem wait_ok : waitA ∈ reach ∧ waitB ∈ reach ∧ (ALab.tick, waitB) ∈ ledges waitA ∧
    (ALab.dG, waitA) ∈ ledges waitB ∧ enabledA ⟨0, by decide⟩ waitA = true ∧
    enabledA ⟨0, by decide⟩ waitB = true := by
  decide +kernel

theorem waitCfg_reach (t : Nat) : waitCfg t ∈ reach := by
  unfold waitCfg; split
  · exact wait_ok.1
  · exact wait_ok.2.1

theorem waitCfg_edge (t : Nat) : (waitL t, waitCfg (t + 1)) ∈ ledges (waitCfg t) := by
  unfold waitCfg waitL
  by_cases h : t % 2 = 0
  · have h' : ¬ (t + 1) % 2 = 0 := by omega
    simp [h, h', wait_ok.2.2.1]
  · have h' : (t + 1) % 2 = 0 := by omega
    simp [h, h', wait_ok.2.2.2.1]

theorem reach_wait : Reachable N (anyEnv N) (ofView waitA) := by
  have h4 := reachable_step (c := (.pending, .vehiclesAmber, [], [])) (d := (.pending, .vehiclesAmber, [], [tk])) (a := .tick)
    reach_pend_amber (by decide +kernel) (by decide +kernel)
  have h5 := reachable_step (c := (.pending, .vehiclesAmber, [], [tk])) (d := (.pending, .walk, [ak], [])) (a := .dG)
    h4 (by decide +kernel) (by decide +kernel)
  have h6 := reachable_step (c := (.pending, .walk, [ak], [])) (d := (.pending, .walk, [ak], [tk])) (a := .tick)
    h5 (by decide +kernel) (by decide +kernel)
  have h7 := reachable_step (c := (.pending, .walk, [ak], [tk])) (d := (.pending, .clearance, [ak], [])) (a := .dG)
    h6 (by decide +kernel) (by decide +kernel)
  have h8 := reachable_step (c := (.pending, .clearance, [ak], [])) (d := (.pending, .clearance, [ak], [tk])) (a := .tick)
    h7 (by decide +kernel) (by decide +kernel)
  exact reachable_step (c := (.pending, .clearance, [ak], [tk])) (d := waitA) (a := .dG)
    h8 (by decide +kernel) (by decide +kernel)

noncomputable abbrev waitTrace : Trace N (anyEnv N) := mkTrace waitCfg waitL waitCfg_reach waitCfg_edge

/-- **Weak fairness is needed.** There is a trace from a reachable configuration in
which timer expiries are accepted again and again and the latch stays pending forever,
because the acknowledgement waiting in the latch's mailbox is never delivered
although its delivery is enabled throughout. That trace is not weakly fair. -/
theorem fairness_needed :
    ∃ tr : Trace N (anyEnv N), Reachable N (anyEnv N) (tr.cfg 0) ∧ Recurs N tr IsTimerSend ∧
      (∀ t, latch (tr.cfg t) = .pending) ∧ ¬ WeaklyFair N tr := by
  refine ⟨waitTrace, reach_wait, ?_, ?_, ?_⟩
  · apply mkTrace_recurs
    intro t
    exact ⟨t + t % 2, by omega, by have : (t + t % 2) % 2 = 0 := by omega
                                   simp [waitL, this]⟩
  · intro t
    show latch (ofView (waitCfg t)) = _
    unfold waitCfg; split <;> rfl
  · intro hwf
    obtain ⟨s, _, hs⟩ := hwf 0 ⟨0, by decide⟩ (fun s _ => by
      have := enabled_real (k := ofView (waitCfg s)) (by rw [view_ofView]; exact waitCfg_reach s) ⟨0, by decide⟩
        (by rw [view_ofView]; unfold waitCfg; split
            · exact wait_ok.2.2.2.2.1
            · exact wait_ok.2.2.2.2.2)
      exact this)
    have h1 := (alab_eq_labOf _ ⟨0, by decide⟩).mpr hs
    rw [mkTrace_alab (a := waitCfg) (lb := waitL)] at h1
    unfold waitL labOf at h1; split at h1 <;> simp at h1

/-! ## 4. Finite executions are not silently excluded -/

/-- **A request can be outstanding with nothing left to deliver.** A reachable
configuration has a pending request and every mailbox empty, so no delivery is enabled
and a round-robin scheduler reports `Settled`. Only the environment (a timer
expiry) can move it. An execution that stops here is a finite execution; it is not a
trace and `pending_request_acknowledged` does not speak about it: the progress
assumption on the timer is what rules it out. -/
theorem quiescent_pending :
    ∃ k : Config N, Reachable N (anyEnv N) k ∧ latch k = .pending ∧ Settled N k ∧
      ∀ c, deliver N k c = none := by
  refine ⟨ofView (.pending, .vehiclesAmber, [], []), reach_pend_amber, rfl, ?_, ?_⟩
  · intro c
    rcases child_cases c with rfl | rfl <;> rfl
  · intro c
    apply deliver_empty
    rcases child_cases c with rfl | rfl <;> rfl

theorem reach_has_edge : ∀ c ∈ reach, ledges c ≠ [] := by decide +kernel

/-- **No reachable dead end.** Every reachable configuration has a permitted
successor (a delivery, or a send with room), so no finite prefix of behaviour is
silently excluded from the infinite traces for lack of a continuation. -/
theorem no_dead_end (E : Env N) {k : Config N} (h : Reachable N E k) :
    ∃ k', Step N (anyEnv N) k k' := by
  have hr := view_reachable E h
  have hb := h.bounded N E
  obtain ⟨e, he⟩ := List.exists_mem_of_ne_nil _ (reach_has_edge _ hr)
  obtain ⟨l, k', hs, _, _⟩ := ledges_realized hb (a := e.1) (d := e.2) (by simpa using he)
  exact ⟨k', l, hs⟩

end CrossingV1LivenessWitness
