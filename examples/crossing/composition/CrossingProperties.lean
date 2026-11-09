import L2RDerived.crossing

open L2R.Async
open Crossing

namespace CrossingProperties

abbrev N : Network := L2RDerived.crossing.crossing.network
/-- Scheduler index of `request_latch`. -/
abbrev L : Fin 2 := 0
/-- Scheduler index of `signal`. -/
abbrev G : Fin 2 := 1

def latch (k : Config N) : Latch.State := k.state L
def signal (k : Config N) : Signal.State := k.state G
def latchIn (k : Config N) : List Latch.Input := k.inbox L
def signalIn (k : Config N) : List Signal.Input := k.inbox G

theorem GL : G ≠ L := by decide
theorem LG : L ≠ G := by decide


/-! Routing, by finite case analysis over the Lean specification. -/

theorem latch_msgs_to_latch (s : Latch.State) (i : Latch.Input) :
    msgsFor N L (emitted N L (N.step L s i).2) = [] := by
  cases s <;> cases i <;> rfl

theorem latch_msgs_to_signal (s : Latch.State) (i : Latch.Input) :
    msgsFor N G (emitted N L (N.step L s i).2) = ((Latch.step s i).2.bind Routes.latchToSignal).toList := by
  cases s <;> cases i <;> rfl

theorem signal_msgs_to_signal (s : Signal.State) (i : Signal.Input) :
    msgsFor N G (emitted N G (N.step G s i).2) = [] := by
  cases s <;> cases i <;> rfl

theorem signal_msgs_to_latch (s : Signal.State) (i : Signal.Input) :
    msgsFor N L (emitted N G (N.step G s i).2) = ((Signal.step s i).2.bind Routes.signalToLatch).toList := by
  cases s <;> cases i <;> rfl

/-- A delivery to the latch, observed through the four components. -/
theorem deliver_latch {k k' : Config N} (h : deliver N k L = some k') :
    ∃ i rest, latchIn k = i :: rest ∧ latch k' = (Latch.step (latch k) i).1 ∧ latchIn k' = rest ∧
      signal k' = signal k ∧
      signalIn k' = signalIn k ++ ((Latch.step (latch k) i).2.bind Routes.latchToSignal).toList := by
  obtain ⟨i, rest, hin, hs, hi⟩ := deliver_some N h
  refine ⟨i, rest, hin, ?_, ?_, ?_, ?_⟩
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

/-- A delivery to the signal, observed through the four components. -/
theorem deliver_signal {k k' : Config N} (h : deliver N k G = some k') :
    ∃ i rest, signalIn k = i :: rest ∧ signal k' = (Signal.step (signal k) i).1 ∧ signalIn k' = rest ∧
      latch k' = latch k ∧
      latchIn k' = latchIn k ++ ((Signal.step (signal k) i).2.bind Routes.signalToLatch).toList := by
  obtain ⟨i, rest, hin, hs, hi⟩ := deliver_some N h
  refine ⟨i, rest, hin, ?_, ?_, ?_, ?_⟩
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

/-! ## The environment

The generated `composite/v0` API (`System::send`) accepts any input for any
child, i.e. `anyInput`. The properties below are proved for the **physical**
environment only: the push button sends presses to the latch and the timer
sends expiries to the signal. They hold for a caller of the generated API only
if it respects that contract; typed ports (`composite/v1`) are the intended way
to make the architecture itself state it. -/

inductive Physical : (c : Fin N.size) → N.Input c → Prop
  | press : Physical L Latch.Input.buttonPressed
  | timer : Physical G Signal.Input.timerExpired

theorem send_press (k : Config N) :
    latch (post N k ⟨L, Latch.Input.buttonPressed⟩) = latch k ∧
    latchIn (post N k ⟨L, Latch.Input.buttonPressed⟩) = latchIn k ++ [.buttonPressed] ∧
    signal (post N k ⟨L, Latch.Input.buttonPressed⟩) = signal k ∧
    signalIn (post N k ⟨L, Latch.Input.buttonPressed⟩) = signalIn k :=
  ⟨rfl, post_inbox_same N k L _, rfl, post_inbox_other N k _ GL⟩

theorem send_timer (k : Config N) :
    latch (post N k ⟨G, Signal.Input.timerExpired⟩) = latch k ∧
    latchIn (post N k ⟨G, Signal.Input.timerExpired⟩) = latchIn k ∧
    signal (post N k ⟨G, Signal.Input.timerExpired⟩) = signal k ∧
    signalIn (post N k ⟨G, Signal.Input.timerExpired⟩) = signalIn k ++ [.timerExpired] :=
  ⟨rfl, post_inbox_other N k _ LG, rfl, post_inbox_same N k G _⟩

theorem child_cases (c : Fin N.size) : c = L ∨ c = G :=
  match c with
  | ⟨0, _⟩ => Or.inl rfl
  | ⟨1, _⟩ => Or.inr rfl
  | ⟨n + 2, h⟩ => absurd h (by change ¬ n + 2 < 2; omega)

/-! ## Safety over every permitted interleaving -/

/-- Pedestrian-request tokens in the system: requests travelling to the signal,
the signal's amber phase (a request being served), and acknowledgements
travelling back to the latch. -/
def inflight (k : Config N) : Nat :=
  (signalIn k).count .crossingRequested + (if signal k = .vehiclesAmber then 1 else 0) +
    (latchIn k).count .acknowledged

/-- At most one request is in the system, and while one is, the latch is pending. -/
def Coherent (k : Config N) : Prop :=
  inflight k ≤ 1 ∧ (inflight k = 1 → latch k = .pending)

theorem coherent_step (k k' : Config N) (hk : Coherent k) (h : Step N Physical k k') :
    Coherent k' := by
  unfold Coherent inflight at *
  cases h with
  | send c i he =>
    cases he with
    | press =>
      obtain ⟨h1, h2, h3, h4⟩ := send_press k
      rw [h1, h2, h3, h4]; simpa [List.count_append] using hk
    | timer =>
      obtain ⟨h1, h2, h3, h4⟩ := send_timer k
      rw [h1, h2, h3, h4]; simpa [List.count_append] using hk
  | deliver c hd =>
    rcases child_cases c with rfl | rfl
    · obtain ⟨i, rest, hin, h1, h2, h3, h4⟩ := deliver_latch hd
      rw [h1, h2, h3, h4]
      rw [hin] at hk
      revert hk
      cases latch k <;> cases i <;>
        simp [Latch.step, Routes.latchToSignal, List.count_append] <;>
        first | omega | (intro a b; exact ⟨by omega, fun e => b (by omega)⟩)
    · obtain ⟨i, rest, hin, h1, h2, h3, h4⟩ := deliver_signal hd
      rw [h1, h2, h3, h4]
      rw [hin] at hk
      revert hk
      cases signal k <;> cases i <;>
        simp [Signal.step, Routes.signalToLatch, List.count_append] <;>
        first | omega | (intro a b; exact ⟨by omega, fun e => b (by omega)⟩)

theorem initial_coherent : Coherent (initial N) := by
  unfold Coherent inflight; decide

/-- **System safety, all interleavings.** Every configuration that any permitted
behaviour reaches in the physical environment is coherent. -/
theorem coherent_of_reachable : ∀ k, Reachable N Physical k → Coherent k :=
  reachable_invariant N Physical Coherent initial_coherent fun k k' _ hk h => coherent_step k k' hk h

/-- At most one pedestrian request travels to the signal at any time. -/
theorem at_most_one_request (k : Config N) (h : Reachable N Physical k) :
    (signalIn k).count .crossingRequested ≤ 1 := by
  have := (coherent_of_reachable k h).1; unfold inflight at this; omega

/-- An acknowledgement only ever travels to a pending latch. -/
theorem acknowledgement_only_to_pending (k : Config N) (h : Reachable N Physical k)
    (ha : 0 < (latchIn k).count .acknowledged) : latch k = .pending := by
  have ⟨h1, h2⟩ := coherent_of_reachable k h
  unfold inflight at h1 h2
  exact h2 (by omega)

/-- A per-machine invariant lifts to the composition (the routine case). -/
theorem no_conflicting_release (k : Config N) :
    ¬ (Signal.vehiclesMayMove (signal k) = true ∧ Signal.pedestriansMayWalk (signal k) = true) :=
  Signal.no_conflicting_release (signal k)

/-- **The generated scheduler.** Each property above holds in every
configuration the `composite/v0` round-robin scheduler reaches, because every
scheduler run is a permitted behaviour (`RoundRobin.run_sound`). The link from
this Lean scheduler model to the generated Rust is reviewed, not proved. -/
theorem scheduler_coherent {s : RoundRobin.Sched N} (h : RoundRobin.Run N Physical s) :
    Coherent s.cfg :=
  RoundRobin.run_invariant N Physical Coherent coherent_of_reachable h

/-! ## Liveness fails: a request can be stranded

`Signal.step` ignores `crossingRequested` outside the green phase, and the latch
coalesces presses while pending. If a request reaches the signal after it has
left green, it is dropped while the latch stays pending, so every later press is
silent and no request is ever made again. This is a property of the
composition, not of either machine: both machines' own theorems hold. -/

/-- The latch waits, and nothing that could ever acknowledge it is in the system. -/
def Stranded (k : Config N) : Prop := latch k = .pending ∧ inflight k = 0

theorem stranded_step (k k' : Config N) (hk : Stranded k) (h : Step N Physical k k') :
    Stranded k' := by
  unfold Stranded inflight at *
  cases h with
  | send c i he =>
    cases he with
    | press =>
      obtain ⟨h1, h2, h3, h4⟩ := send_press k
      rw [h1, h2, h3, h4]; simpa [List.count_append] using hk
    | timer =>
      obtain ⟨h1, h2, h3, h4⟩ := send_timer k
      rw [h1, h2, h3, h4]; simpa [List.count_append] using hk
  | deliver c hd =>
    rcases child_cases c with rfl | rfl
    · obtain ⟨i, rest, hin, h1, h2, h3, h4⟩ := deliver_latch hd
      rw [h1, h2, h3, h4]
      rw [hin] at hk
      revert hk
      cases latch k <;> cases i <;>
        simp [Latch.step, Routes.latchToSignal, List.count_append] <;>
        first | omega | (intro a b; exact ⟨by omega, fun e => b (by omega)⟩)
    · obtain ⟨i, rest, hin, h1, h2, h3, h4⟩ := deliver_signal hd
      rw [h1, h2, h3, h4]
      rw [hin] at hk
      revert hk
      cases signal k <;> cases i <;>
        simp [Signal.step, Routes.signalToLatch, List.count_append] <;>
        first | omega | (intro a b; exact ⟨by omega, fun e => b (by omega)⟩)

/-- From a stranded configuration, **no** continuation in the physical
environment ever acknowledges the latch, whatever the order of deliveries, so
no fairness assumption can rescue it. -/
theorem stranded_forever (k k' : Config N) (hk : Stranded k) (h : Star N Physical k k') :
    latch k' = .pending :=
  (star_invariant N Physical Stranded stranded_step k k' hk h).1

/-- The round-robin scheduler reaches a stranded configuration: press, serve the
latch, serve the signal (amber), timer, serve the signal (walk; acknowledgement
sent), serve the latch (idle), press, serve the latch (request sent), serve the
signal (in walk: request dropped). -/
theorem scheduler_strands_a_request :
    ∃ s, RoundRobin.Run N Physical s ∧ Stranded s.cfg := by
  have r0 := RoundRobin.Run.start (N := N) (E := Physical)
  have r1 := RoundRobin.Run.send L _ r0 Physical.press
  have r2 := RoundRobin.Run.step r1 rfl
  have r3 := RoundRobin.Run.step r2 rfl
  have r4 := RoundRobin.Run.send G _ r3 Physical.timer
  have r5 := RoundRobin.Run.step r4 rfl
  have r6 := RoundRobin.Run.step r5 rfl
  have r7 := RoundRobin.Run.send L _ r6 Physical.press
  have r8 := RoundRobin.Run.step r7 rfl
  have r9 := RoundRobin.Run.step r8 rfl
  exact ⟨_, r9, by unfold Stranded inflight; decide⟩

/-- **Liveness does not hold.** In the physical environment, the composed
system can reach a configuration with a pending pedestrian request that no
permitted continuation ever acknowledges, and the generated scheduler reaches it. -/
theorem request_can_be_stranded :
    ∃ k, Reachable N Physical k ∧ latch k = .pending ∧
      ∀ k', Star N Physical k k' → latch k' = .pending := by
  obtain ⟨s, hr, hs⟩ := scheduler_strands_a_request
  exact ⟨s.cfg, RoundRobin.run_sound N Physical hr, hs.1, fun k' h => stranded_forever _ k' hs h⟩

end CrossingProperties
