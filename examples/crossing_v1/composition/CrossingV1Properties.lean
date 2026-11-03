import L2RDerived.crossing_v1

/-!
# System properties of the bounded, repaired crossing (`composite/v1`)

What is proved here, over **every** permitted interleaving of the derived
network (`L2RDerived/crossing_v1.lean`, semantics `l2r.async/v1`):

* **Structural interface.** Safety holds for *every* environment `E`, including
  one that sends whenever it likes. The only messages the environment can add are
  `ButtonEvent`s to the latch and `TimerEvent`s to the signal (the typed ports),
  so the unrepresentable injections of the v0 example are not an assumption any
  more. When events may occur (the content of `E`) does not matter for these.
* **Coherence.** At most one pedestrian request is in the system, and the latch is
  pending exactly while one is.
* **Capacity.** No mailbox ever exceeds its capacity; nothing is lost to overflow
  (the semantics has no such path).
* **No deadlock.** No reachable configuration is `Blocked`: whenever a message is
  pending, some delivery is enabled.
* **The scheduler.** All of the above hold for every run of the generated
  round-robin scheduler (`RoundRobin.run_sound`).

The method: an inductive invariant over a finite abstraction, checked by kernel
`decide` (no `native_decide`, no new axioms). The abstraction `view` projects a
configuration to its two machine states and two mailbox contents; a permitted
step is *simulated* by the finite successor function `succs`; the finite set
`reach` is closed under it and contains the initial configuration; properties
are decided over `reach`. Liveness is **not** established here; see the
non-stranding and fairness sections for exactly what is and is not claimed.
-/

open L2R.Async1
open L2R.Async (update update_same update_other)
open CrossingV1

namespace CrossingV1Properties

abbrev N : Network := L2RDerived.crossing_v1.crossing.network
abbrev L : Fin 2 := 0
abbrev G : Fin 2 := 1

def latch (k : Config N) : Latch.State := k.state L
def signal (k : Config N) : Signal.State := k.state G
def latchIn (k : Config N) : List Latch.Input := k.inbox L
def signalIn (k : Config N) : List Signal.Input := k.inbox G

theorem GL : G ≠ L := by decide
theorem LG : L ≠ G := by decide

theorem child_cases (c : Fin N.size) : c = L ∨ c = G :=
  match c with
  | ⟨0, _⟩ => Or.inl rfl
  | ⟨1, _⟩ => Or.inr rfl
  | ⟨n + 2, h⟩ => absurd h (by change ¬ n + 2 < 2; omega)

theorem port_cases (p : Fin N.ports) : p = ⟨0, by decide⟩ ∨ p = ⟨1, by decide⟩ :=
  match p with
  | ⟨0, _⟩ => Or.inl rfl
  | ⟨1, _⟩ => Or.inr rfl
  | ⟨n + 2, h⟩ => absurd h (by change ¬ n + 2 < 2; omega)

theorem cap_latch : N.cap L = 2 := rfl
theorem cap_signal : N.cap G = 2 := rfl

/-! ## Routing, by finite case analysis over the Lean specification -/

theorem latch_msgs_to_latch (s : Latch.State) (i : Latch.Input) :
    msgsFor N L (emitted N L (N.step L s i).2) = [] := by
  cases s <;> cases i <;> rfl

theorem latch_msgs_to_signal (s : Latch.State) (i : Latch.Input) :
    msgsFor N G (emitted N L (N.step L s i).2) =
      ((Latch.step s i).2.bind Routes.latchToSignal).toList := by
  cases s <;> cases i <;> rfl

theorem signal_msgs_to_signal (s : Signal.State) (i : Signal.Input) :
    msgsFor N G (emitted N G (N.step G s i).2) = [] := by
  cases s <;> cases i <;> rfl

theorem signal_msgs_to_latch (s : Signal.State) (i : Signal.Input) :
    msgsFor N L (emitted N G (N.step G s i).2) =
      ((Signal.step s i).2.bind Routes.signalToLatch).toList := by
  cases s <;> cases i <;> rfl

/-- A delivery to the latch, observed through the four components. -/
theorem deliver_latch {k k' : Config N} (h : deliver N k L = some k') :
    ∃ i rest, latchIn k = i :: rest ∧ latch k' = (Latch.step (latch k) i).1 ∧ latchIn k' = rest ∧
      signal k' = signal k ∧
      signalIn k' = signalIn k ++ ((Latch.step (latch k) i).2.bind Routes.latchToSignal).toList ∧
      (signalIn k).length + ((Latch.step (latch k) i).2.bind Routes.latchToSignal).toList.length ≤ 2 := by
  obtain ⟨i, rest, hin, hs, hi, hroom⟩ := deliver_some N h
  refine ⟨i, rest, hin, ?_, ?_, ?_, ?_, ?_⟩
  · show k'.state L = _
    rw [hs]; exact update_same _ _ _
  · show k'.inbox L = rest
    have m := latch_msgs_to_latch (k.state L) i
    rw [hi, update_same, m, List.append_nil]
  · show k'.state G = k.state G
    rw [hs]; exact update_other _ _ GL
  · show k'.inbox G = k.inbox G ++ _
    have m := latch_msgs_to_signal (k.state L) i
    rw [hi, update_other _ _ GL, m]; rfl
  · have m := latch_msgs_to_signal (k.state L) i
    have := hroom G
    rw [update_other _ _ GL, m] at this
    exact this

/-- A delivery to the signal, observed through the four components. -/
theorem deliver_signal {k k' : Config N} (h : deliver N k G = some k') :
    ∃ i rest, signalIn k = i :: rest ∧ signal k' = (Signal.step (signal k) i).1 ∧ signalIn k' = rest ∧
      latch k' = latch k ∧
      latchIn k' = latchIn k ++ ((Signal.step (signal k) i).2.bind Routes.signalToLatch).toList ∧
      (latchIn k).length + ((Signal.step (signal k) i).2.bind Routes.signalToLatch).toList.length ≤ 2 := by
  obtain ⟨i, rest, hin, hs, hi, hroom⟩ := deliver_some N h
  refine ⟨i, rest, hin, ?_, ?_, ?_, ?_, ?_⟩
  · show k'.state G = _
    rw [hs]; exact update_same _ _ _
  · show k'.inbox G = rest
    have m := signal_msgs_to_signal (k.state G) i
    rw [hi, update_same, m, List.append_nil]
  · show k'.state L = k.state L
    rw [hs]; exact update_other _ _ LG
  · show k'.inbox L = k.inbox L ++ _
    have m := signal_msgs_to_latch (k.state G) i
    rw [hi, update_other _ _ LG, m]; rfl
  · have m := signal_msgs_to_latch (k.state G) i
    have := hroom L
    rw [update_other _ _ LG, m] at this
    exact this

theorem send_button (k : Config N) :
    latch (inject N k ⟨0, by decide⟩ .pressed) = latch k ∧
    latchIn (inject N k ⟨0, by decide⟩ .pressed) = latchIn k ++ [.buttonPressed] ∧
    signal (inject N k ⟨0, by decide⟩ .pressed) = signal k ∧
    signalIn (inject N k ⟨0, by decide⟩ .pressed) = signalIn k := by
  refine ⟨rfl, ?_, rfl, ?_⟩
  · exact update_same (β := fun c => List (N.Input c)) k.inbox L (k.inbox L ++ ([Latch.Input.buttonPressed] : List (N.Input L)))
  · exact update_other (β := fun c => List (N.Input c)) k.inbox (c := L) (d := G) (k.inbox L ++ ([Latch.Input.buttonPressed] : List (N.Input L))) GL

theorem send_timer (k : Config N) :
    latch (inject N k ⟨1, by decide⟩ .expired) = latch k ∧
    latchIn (inject N k ⟨1, by decide⟩ .expired) = latchIn k ∧
    signal (inject N k ⟨1, by decide⟩ .expired) = signal k ∧
    signalIn (inject N k ⟨1, by decide⟩ .expired) = signalIn k ++ [.timerExpired] := by
  refine ⟨rfl, ?_, rfl, ?_⟩
  · exact update_other (β := fun c => List (N.Input c)) k.inbox (c := G) (d := L) (k.inbox G ++ ([Signal.Input.timerExpired] : List (N.Input G))) LG
  · exact update_same (β := fun c => List (N.Input c)) k.inbox G (k.inbox G ++ ([Signal.Input.timerExpired] : List (N.Input G)))

/-! ## The finite abstraction

Everything that matters for the system properties is the two machine states and
the two mailbox contents. The mailboxes hold at most two messages of two kinds
each, so there are finitely many such quadruples. -/

abbrev CCfg := Latch.State × Signal.State × List Latch.Input × List Signal.Input

def view (k : Config N) : CCfg := (latch k, signal k, latchIn k, signalIn k)

/-- Delivery to the latch on the abstraction: enabled only with room for its output. -/
def cdeliverL : CCfg → Option CCfg
  | (ls, ss, i :: rest, si) =>
    if si.length + ((Latch.step ls i).2.bind Routes.latchToSignal).toList.length ≤ 2 then
      some ((Latch.step ls i).1, ss, rest, si ++ ((Latch.step ls i).2.bind Routes.latchToSignal).toList)
    else none
  | _ => none

/-- Delivery to the signal on the abstraction. -/
def cdeliverG : CCfg → Option CCfg
  | (ls, ss, li, i :: rest) =>
    if li.length + ((Signal.step ss i).2.bind Routes.signalToLatch).toList.length ≤ 2 then
      some (ls, (Signal.step ss i).1, li ++ ((Signal.step ss i).2.bind Routes.signalToLatch).toList, rest)
    else none
  | _ => none

/-- Port sends on the abstraction: only the two typed ports, only with room. -/
def csends : CCfg → List CCfg
  | (ls, ss, li, si) =>
    (if li.length < 2 then [(ls, ss, li ++ [Latch.Input.buttonPressed], si)] else []) ++
    (if si.length < 2 then [(ls, ss, li, si ++ [Signal.Input.timerExpired])] else [])

def succs (c : CCfg) : List CCfg := (cdeliverL c).toList ++ (cdeliverG c).toList ++ csends c

theorem sim_deliver_latch {k k' : Config N} (h : deliver N k L = some k') :
    cdeliverL (view k) = some (view k') := by
  obtain ⟨i, rest, hin, h1, h2, h3, h4, hroom⟩ := deliver_latch h
  simp only [view, hin, cdeliverL]
  simp only [hroom, ite_true]
  simp only [view, h1, h2, h3, h4]

theorem sim_deliver_signal {k k' : Config N} (h : deliver N k G = some k') :
    cdeliverG (view k) = some (view k') := by
  obtain ⟨i, rest, hin, h1, h2, h3, h4, hroom⟩ := deliver_signal h
  simp only [view, hin, cdeliverG]
  simp only [hroom, ite_true]
  simp only [view, h1, h2, h3, h4]

/-- **Simulation.** Every permitted step, in any environment, is a step of the
finite abstraction. -/
theorem step_sim (E : Env N) {k k' : Config N} (h : Step N E k k') : view k' ∈ succs (view k) := by
  obtain ⟨l, hl⟩ := h
  cases hl with
  | deliver c hd =>
    rcases child_cases c with rfl | rfl
    · have := sim_deliver_latch hd
      simp [succs, this]
    · have := sim_deliver_signal hd
      simp [succs, this]
  | send p m _ hroom =>
    rcases port_cases p with rfl | rfl
    · cases m
      obtain ⟨a, b, c, d⟩ := send_button k
      have hr : (latchIn k).length < 2 := hroom
      have hv : view (inject N k ⟨0, by decide⟩ .pressed) =
          (latch k, signal k, latchIn k ++ [Latch.Input.buttonPressed], signalIn k) := by
        simp only [view, a, b, c, d]
      rw [hv]
      simp [succs, csends, view, hr]
    · cases m
      obtain ⟨a, b, c, d⟩ := send_timer k
      have hr : (signalIn k).length < 2 := hroom
      have hv : view (inject N k ⟨1, by decide⟩ .expired) =
          (latch k, signal k, latchIn k, signalIn k ++ [Signal.Input.timerExpired]) := by
        simp only [view, a, b, c, d]
      rw [hv]
      simp [succs, csends, view, hr]

/-! ## The reachable set of the abstraction, and its closure -/

def step_all (seen : List CCfg) : List CCfg :=
  seen ++ ((seen.flatMap succs).filter fun c => c ∉ seen).eraseDups

def closure : Nat → List CCfg → List CCfg
  | 0, seen => seen
  | n + 1, seen =>
    let next := step_all seen
    if next.length = seen.length then seen else closure n next

def initialC : CCfg := (Latch.State.idle, Signal.State.vehiclesGreen, [], [])

def reach : List CCfg := closure 40 [initialC]

theorem reach_has_initial : initialC ∈ reach := by decide +kernel

theorem reach_closed : ∀ c ∈ reach, ∀ d ∈ succs c, d ∈ reach := by decide +kernel

theorem view_initial : view (initial N) = initialC := rfl

/-- Every configuration any permitted behaviour reaches, in **any** environment,
abstracts to a member of the finite set `reach`. -/
theorem view_reachable (E : Env N) {k : Config N} (h : Reachable N E k) : view k ∈ reach := by
  induction h with
  | init => rw [view_initial]; exact reach_has_initial
  | step _ hs ih => exact reach_closed _ ih _ (step_sim E hs)

/-! ## Safety, capacity and absence of deadlock -/

/-- Pedestrian-request tokens: requests travelling to the signal, a request held
by the signal (amber, or remembered during walk or clearance), and
acknowledgements travelling back to the latch. -/
def tokens : CCfg → Nat
  | (_, ss, li, si) =>
    si.count .crossingRequested +
      (match ss with
        | .vehiclesAmber | .walkQueued | .clearanceQueued => 1
        | _ => 0) +
      li.count .acknowledged

/-- At most one request is in the system, and the latch is pending exactly while one is. -/
def CCoherent (c : CCfg) : Prop :=
  tokens c ≤ 1 ∧ (tokens c = 1 ↔ c.1 = .pending)

instance : DecidablePred CCoherent := fun c => by unfold CCoherent; infer_instance

theorem reach_coherent : ∀ c ∈ reach, CCoherent c := by decide +kernel

theorem reach_bounded : ∀ c ∈ reach, c.2.2.1.length ≤ 2 ∧ c.2.2.2.length ≤ 2 := by decide +kernel

/-- Some delivery is enabled whenever a message is pending. -/
def Live (c : CCfg) : Bool :=
  (c.2.2.1.isEmpty && c.2.2.2.isEmpty) || (cdeliverL c).isSome || (cdeliverG c).isSome

theorem reach_live : ∀ c ∈ reach, Live c = true := by decide +kernel

def Coherent (k : Config N) : Prop := CCoherent (view k)

/-- **System safety, every environment.** In every configuration any permitted
behaviour reaches, at most one pedestrian request is in the system, and the
latch is pending exactly while one is. `E` is arbitrary: the interface offers the
environment only the two typed ports, so no admissibility assumption is needed. -/
theorem coherent_of_reachable (E : Env N) {k : Config N} (h : Reachable N E k) : Coherent k :=
  reach_coherent _ (view_reachable E h)

theorem tokens_view (k : Config N) :
    tokens (view k) = (signalIn k).count .crossingRequested +
      (match signal k with
        | .vehiclesAmber | .walkQueued | .clearanceQueued => 1
        | _ => 0) + (latchIn k).count .acknowledged := rfl

/-- At most one pedestrian request travels to the signal at any time. -/
theorem at_most_one_request (E : Env N) {k : Config N} (h : Reachable N E k) :
    (signalIn k).count .crossingRequested ≤ 1 := by
  have := (coherent_of_reachable E h).1
  rw [tokens_view] at this
  omega

/-- An acknowledgement only ever travels to a pending latch. -/
theorem acknowledgement_only_to_pending (E : Env N) {k : Config N} (h : Reachable N E k)
    (ha : 0 < (latchIn k).count .acknowledged) : latch k = .pending := by
  have hc := coherent_of_reachable E h
  have h1 := hc.1
  have h2 : tokens (view k) = 1 ↔ latch k = .pending := hc.2
  rw [tokens_view] at h1 h2
  exact h2.mp (by omega)

/-- The mailboxes never exceed their capacity (the generic theorem, specialised). -/
theorem mailboxes_bounded (E : Env N) {k : Config N} (h : Reachable N E k) :
    (latchIn k).length ≤ 2 ∧ (signalIn k).length ≤ 2 := by
  have hb := h.bounded N E
  exact ⟨hb L, hb G⟩

/-- A per-machine invariant lifts to the composition (the routine case). -/
theorem no_conflicting_release (k : Config N) :
    ¬ (Signal.vehiclesMayMove (signal k) = true ∧ Signal.pedestriansMayWalk (signal k) = true) :=
  Signal.no_conflicting_release (signal k)

/-! ### No deadlock

Reverse simulation for deliveries: if the abstraction says a delivery is
enabled, it is. -/

theorem enabled_latch {k : Config N} (hb : Bounded N k) {c : CCfg} (hc : view k = c)
    (he : (cdeliverL c).isSome) : (deliver N k L).isSome := by
  cases hl : k.inbox L with
  | nil =>
    have hl0 : latchIn k = [] := hl
    simp [view, hl0] at hc; subst hc; simp [cdeliverL] at he
  | cons i rest =>
    have hl0 : latchIn k = i :: rest := hl
    have hroom : (signalIn k).length + ((Latch.step (latch k) i).2.bind Routes.latchToSignal).toList.length ≤ 2 := by
      subst hc
      simp only [view, hl0, cdeliverL] at he
      split at he
      · assumption
      · simp at he
    have hfits : fits N ⟨update k.state L (N.step L (k.state L) i).1, update k.inbox L rest⟩
        (emitted N L (N.step L (k.state L) i).2) = true := by
      rw [fits_iff]
      intro d
      rcases child_cases d with rfl | rfl
      · have e : msgsFor N L (emitted N L (N.step L (k.state L) i).2) = [] :=
          latch_msgs_to_latch (k.state L) i
        have hbL := hb L
        rw [hl] at hbL
        show (update k.inbox L rest L).length + (msgsFor N L (emitted N L (N.step L (k.state L) i).2)).length ≤ N.cap L
        rw [e, update_same]
        simp at hbL ⊢
        omega
      · have e : msgsFor N G (emitted N L (N.step L (k.state L) i).2) =
            ((Latch.step (latch k) i).2.bind Routes.latchToSignal).toList :=
          latch_msgs_to_signal (k.state L) i
        show (update k.inbox L rest G).length + (msgsFor N G (emitted N L (N.step L (k.state L) i).2)).length ≤ N.cap G
        rw [e, update_other _ _ GL]
        exact hroom
    unfold deliver
    rw [hl]
    dsimp only
    simp only [hfits, ite_true]
    rfl

theorem enabled_signal {k : Config N} (hb : Bounded N k) {c : CCfg} (hc : view k = c)
    (he : (cdeliverG c).isSome) : (deliver N k G).isSome := by
  cases hl : k.inbox G with
  | nil =>
    have hl0 : signalIn k = [] := hl
    simp [view, hl0] at hc; subst hc; simp [cdeliverG] at he
  | cons i rest =>
    have hl0 : signalIn k = i :: rest := hl
    have hroom : (latchIn k).length + ((Signal.step (signal k) i).2.bind Routes.signalToLatch).toList.length ≤ 2 := by
      subst hc
      simp only [view, hl0, cdeliverG] at he
      split at he
      · assumption
      · simp at he
    have hfits : fits N ⟨update k.state G (N.step G (k.state G) i).1, update k.inbox G rest⟩
        (emitted N G (N.step G (k.state G) i).2) = true := by
      rw [fits_iff]
      intro d
      rcases child_cases d with rfl | rfl
      · have e : msgsFor N L (emitted N G (N.step G (k.state G) i).2) =
            ((Signal.step (signal k) i).2.bind Routes.signalToLatch).toList :=
          signal_msgs_to_latch (k.state G) i
        show (update k.inbox G rest L).length + (msgsFor N L (emitted N G (N.step G (k.state G) i).2)).length ≤ N.cap L
        rw [e, update_other _ _ LG]
        exact hroom
      · have e : msgsFor N G (emitted N G (N.step G (k.state G) i).2) = [] :=
          signal_msgs_to_signal (k.state G) i
        have hbG := hb G
        rw [hl] at hbG
        show (update k.inbox G rest G).length + (msgsFor N G (emitted N G (N.step G (k.state G) i).2)).length ≤ N.cap G
        rw [e, update_same]
        simp at hbG ⊢
        omega
    unfold deliver
    rw [hl]
    dsimp only
    simp only [hfits, ite_true]
    rfl

/-- **No deadlock, every environment.** No reachable configuration is blocked:
whenever a message is pending, some delivery is enabled. -/
theorem no_deadlock (E : Env N) {k : Config N} (h : Reachable N E k) : ¬ Blocked N k := by
  rintro ⟨⟨c, hc⟩, hall⟩
  have hlive := reach_live _ (view_reachable E h)
  have hb := h.bounded N E
  have hLn : deliver N k L = none := hall L
  have hGn : deliver N k G = none := hall G
  simp only [Live, Bool.or_eq_true, Bool.and_eq_true, List.isEmpty_iff] at hlive
  rcases hlive with (⟨h1, h2⟩ | hl) | hg
  · rcases child_cases c with rfl | rfl
    · exact hc h1
    · exact hc h2
  · have := enabled_latch hb rfl hl
    rw [hLn] at this; simp at this
  · have := enabled_signal hb rfl hg
    rw [hGn] at this; simp at this

/-! ## No stranding

The original example can reach a configuration from which *no* continuation ever
acknowledges a pending latch (`request_can_be_stranded` in
`examples/crossing`). This section proves the opposite for the repaired
specification: from **every** reachable configuration with a pending latch, some
permitted continuation acknowledges it.

**What this is, and is not.** It is *non-stranding*: an acknowledgement stays
possible. It is **not** liveness: it does not say the acknowledgement happens.
That needs fairness hypotheses about the scheduler and the environment (the
timer must keep expiring), and is not proved here. The continuation uses the
environment's cooperation (timer expiries), so the environment is `anyEnv`, the
weakest one, which permits every send that has room. -/

theorem view_send_button (k : Config N) :
    view (inject N k ⟨0, by decide⟩ .pressed) =
      (latch k, signal k, latchIn k ++ [Latch.Input.buttonPressed], signalIn k) := by
  obtain ⟨a, b, c, d⟩ := send_button k
  simp only [view, a, b, c, d]

theorem view_send_timer (k : Config N) :
    view (inject N k ⟨1, by decide⟩ .expired) =
      (latch k, signal k, latchIn k, signalIn k ++ [Signal.Input.timerExpired]) := by
  obtain ⟨a, b, c, d⟩ := send_timer k
  simp only [view, a, b, c, d]

/-- Reverse simulation: every abstract successor of a bounded configuration is
realised by a permitted step. -/
theorem succs_realized {k : Config N} (hb : Bounded N k) {d : CCfg} (hd : d ∈ succs (view k)) :
    ∃ k', Step N (anyEnv N) k k' ∧ view k' = d := by
  simp only [succs, List.mem_append, Option.mem_toList] at hd
  rcases hd with (hl | hg) | hs
  · have he : (cdeliverL (view k)).isSome := by simp [hl]
    obtain ⟨k', hk'⟩ := Option.isSome_iff_exists.mp (enabled_latch hb rfl he)
    have := sim_deliver_latch hk'
    rw [hl] at this
    exact ⟨k', ⟨.deliver L, StepL.deliver (N := N) (E := anyEnv N) L hk'⟩, (Option.some.inj this).symm⟩
  · have he : (cdeliverG (view k)).isSome := by simp [hg]
    obtain ⟨k', hk'⟩ := Option.isSome_iff_exists.mp (enabled_signal hb rfl he)
    have := sim_deliver_signal hk'
    rw [hg] at this
    exact ⟨k', ⟨.deliver G, StepL.deliver (N := N) (E := anyEnv N) G hk'⟩, (Option.some.inj this).symm⟩
  · simp only [csends, view, List.mem_append] at hs
    rcases hs with hs | hs
    · by_cases hr : (latchIn k).length < 2
      · simp only [hr, ite_true, List.mem_singleton] at hs
        refine ⟨inject N k ⟨0, by decide⟩ .pressed,
          ⟨.send ⟨0, by decide⟩ .pressed, StepL.send k ⟨0, by decide⟩ .pressed trivial hr⟩, ?_⟩
        rw [view_send_button, hs]
      · simp [hr] at hs
    · by_cases hr : (signalIn k).length < 2
      · simp only [hr, ite_true, List.mem_singleton] at hs
        refine ⟨inject N k ⟨1, by decide⟩ .expired,
          ⟨.send ⟨1, by decide⟩ .expired, StepL.send k ⟨1, by decide⟩ .expired trivial hr⟩, ?_⟩
        rw [view_send_timer, hs]
      · simp [hr] at hs

/-- Layers of the abstraction by distance (in permitted steps) to an idle latch. -/
def layers : Nat → List (List CCfg)
  | 0 => [reach.filter fun c => c.1 == .idle]
  | n + 1 =>
    let prev := layers n
    let seen := prev.flatten
    prev ++ [reach.filter fun c => c ∉ seen && (succs c).any (· ∈ seen)]

def dist (c : CCfg) : Nat := ((layers 14).findIdx fun l => c ∈ l)

/-- The certificate: every pending, reachable abstract configuration has a successor
strictly closer to an idle latch, and every idle one is at distance 0. -/
theorem dist_certificate : ∀ c ∈ reach,
    dist c < (layers 14).length ∧
    (c.1 = .idle → dist c = 0) ∧
    (c.1 = .pending → ∃ d ∈ succs c, dist d < dist c) := by
  decide +kernel

theorem star_trans {E : Env N} {a b c : Config N} (h1 : Star N E a b) (h2 : Star N E b c) :
    Star N E a c := by
  induction h2 with
  | refl => exact h1
  | tail _ hs ih => exact Star.tail ih hs

/-- **No stranding.** In every environment's reachable configurations, whenever
the latch is pending, some permitted continuation (with a cooperative
environment) leaves it idle: the request is never lost for good. -/
theorem no_stranding (E : Env N) {k : Config N} (h : Reachable N E k) (hp : latch k = .pending) :
    ∃ k', Star N (anyEnv N) k k' ∧ latch k' = .idle := by
  have key : ∀ n, ∀ k : Config N, Bounded N k → view k ∈ reach → dist (view k) = n →
      latch k = .pending → ∃ k', Star N (anyEnv N) k k' ∧ latch k' = .idle := by
    intro n
    induction n using Nat.strongRecOn with
    | _ n ih =>
      intro k hb hr hd hpend
      obtain ⟨_, _, hstep⟩ := dist_certificate _ hr
      obtain ⟨d, hdmem, hlt⟩ := hstep (by simpa [view] using hpend)
      obtain ⟨k', hk', hv⟩ := succs_realized hb hdmem
      have hb' := step_bounded N (anyEnv N) hb hk'
      have hr' : view k' ∈ reach := by rw [hv]; exact reach_closed _ hr _ hdmem
      by_cases hidle : latch k' = .idle
      · exact ⟨k', Star.tail (Star.refl k) hk', hidle⟩
      · have hp' : latch k' = .pending := by
          cases hl : latch k' with
          | idle => exact absurd hl hidle
          | pending => rfl
        obtain ⟨k'', hs, hl⟩ := ih (dist (view k')) (by rw [hv]; omega) k' hb' hr' rfl hp'
        exact ⟨k'', star_trans (Star.tail (Star.refl k) hk') hs, hl⟩
  exact key _ k (h.bounded N E) (view_reachable E h) rfl hp

/-! ## The generated scheduler

Every property above holds in every configuration the `composite/v1`
round-robin scheduler reaches, because every scheduler run is a permitted
behaviour (`RoundRobin.run_sound`), for any caller. The link from this Lean
scheduler model to the generated Rust is established as described in
`docs/verification.md`, not by this file. -/

theorem scheduler_coherent (E : Env N) {s : RoundRobin.Sched N} (h : RoundRobin.Run N E s) :
    Coherent s.cfg :=
  coherent_of_reachable E (RoundRobin.run_sound N E h)

theorem scheduler_bounded (E : Env N) {s : RoundRobin.Sched N} (h : RoundRobin.Run N E s) :
    Bounded N s.cfg :=
  RoundRobin.run_bounded N E h

/-- The generated scheduler never reports `Blocked`: with messages pending it always delivers. -/
theorem scheduler_never_blocks (E : Env N) {s : RoundRobin.Sched N} (h : RoundRobin.Run N E s) :
    RoundRobin.stepOnce N s ≠ .blocked :=
  fun hb => no_deadlock E (RoundRobin.run_sound N E h) (RoundRobin.stepOnce_blocked N s hb)

end CrossingV1Properties
