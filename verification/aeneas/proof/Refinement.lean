import Extracted.CrossingCore
import Crossing

/-!
# The crossing implementation refines its Lean specification

`Extracted.CrossingCore` is the Aeneas translation of the committed Rust
implementation of both machines (`request_latch`, `signal`: generated contract
`mod.rs` plus the implementer's `imp.rs`) and of the message translations
(`routes.rs`). It is regenerated from those exact sources by
`//verification/aeneas:refinement_test`.

For each function this file proves, under the abstraction functions below, that
the translated code returns `ok` (it cannot panic) with exactly the result the
Lean specification gives, for every input. Two specification invariants are then
transferred to the implementation.

**Not covered:** the generated composite scheduler (`crossing/mod.rs`) and any
property of the composed, asynchronous system. A verified transition function
is not a verified system.

Trusted base: the Lean kernel; Charon and Aeneas translating the Rust source
faithfully; rustc compiling that source as Charon read it; the abstraction
functions, which are part of the reviewed statements.
-/

open Aeneas Aeneas.Std Result
open crossing_core

namespace Crossing.Refinement

/-! ## Abstraction functions (bijections between the enumerations) -/

def latchState : request_latch.State → Crossing.Latch.State
  | .Idle => .idle
  | .Pending => .pending

def latchInput : request_latch.Input → Crossing.Latch.Input
  | .ButtonPressed => .buttonPressed
  | .Acknowledged => .acknowledged

def latchOutput : request_latch.Output → Crossing.Latch.Output
  | .Requested => .requested

def signalState : signal.State → Crossing.Signal.State
  | .VehiclesGreen => .vehiclesGreen
  | .VehiclesAmber => .vehiclesAmber
  | .Walk => .walk
  | .Clearance => .clearance

def signalInput : signal.Input → Crossing.Signal.Input
  | .CrossingRequested => .crossingRequested
  | .TimerExpired => .timerExpired

def signalOutput : signal.Output → Crossing.Signal.Output
  | .WalkStarted => .walkStarted

/-! ## Request latch -/

theorem latch_init_refines :
    ∃ s, request_latch.init = ok s ∧ latchState s = Crossing.Latch.init := by
  exact ⟨.Idle, rfl, rfl⟩

theorem latch_step_refines (s : request_latch.State) (i : request_latch.Input) :
    ∃ r, request_latch.step s i = ok r ∧
      (latchState r.1, r.2.map latchOutput) = Crossing.Latch.step (latchState s) (latchInput i) := by
  cases s <;> cases i <;> exact ⟨_, rfl, rfl⟩

/-! ## Signal -/

theorem signal_init_refines :
    ∃ s, signal.init = ok s ∧ signalState s = Crossing.Signal.init := by
  exact ⟨.VehiclesGreen, rfl, rfl⟩

theorem signal_step_refines (s : signal.State) (i : signal.Input) :
    ∃ r, signal.step s i = ok r ∧
      (signalState r.1, r.2.map signalOutput) = Crossing.Signal.step (signalState s) (signalInput i) := by
  cases s <;> cases i <;> exact ⟨_, rfl, rfl⟩

/-! ## Message translations -/

theorem latch_to_signal_refines (o : request_latch.Output) :
    ∃ r, routes.latch_to_signal o = ok r ∧
      r.map signalInput = Crossing.Routes.latchToSignal (latchOutput o) := by
  cases o; exact ⟨_, rfl, rfl⟩

theorem signal_to_latch_refines (o : signal.Output) :
    ∃ r, routes.signal_to_latch o = ok r ∧
      r.map latchInput = Crossing.Routes.signalToLatch (signalOutput o) := by
  cases o; exact ⟨_, rfl, rfl⟩

/-! ## Specification invariants transferred to the implementation -/

/-- The implemented signal enters the walk phase only from amber (or by staying in walk). -/
theorem impl_walk_entered_from_amber (s : signal.State) (i : signal.Input)
    (r : signal.State × Option signal.Output) (h : signal.step s i = ok r)
    (walk : signalState r.1 = .walk) :
    signalState s = .vehiclesAmber ∨ signalState s = .walk := by
  obtain ⟨r', h', agree⟩ := signal_step_refines s i
  rw [h, Result.ok.injEq] at h'
  subst h'
  have : (Crossing.Signal.step (signalState s) (signalInput i)).1 = .walk := by
    rw [← agree]; exact walk
  exact Crossing.Signal.walk_entered_from_amber _ _ this

/-- Every request the implemented latch emits leaves it pending. -/
theorem impl_request_implies_pending (s : request_latch.State) (i : request_latch.Input)
    (r : request_latch.State × Option request_latch.Output) (h : request_latch.step s i = ok r)
    (emitted : r.2 = some .Requested) :
    latchState r.1 = .pending := by
  obtain ⟨r', h', agree⟩ := latch_step_refines s i
  rw [h, Result.ok.injEq] at h'
  subst h'
  have spec := Crossing.Latch.request_implies_pending (latchState s) (latchInput i)
  rw [← agree] at spec
  exact spec (by simp [emitted, latchOutput])

end Crossing.Refinement
