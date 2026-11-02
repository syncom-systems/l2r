import Extracted.SignalCrate
import Crossing.Signal

/-!
# The crossing signal implementation refines its Lean specification

`Extracted.SignalCrate` is the Aeneas translation of the committed Rust
implementation (`examples/crossing/scaffold/src/crossing/signal/imp.rs` and its
generated contract `mod.rs`). This file proves that, under the abstraction
functions below, the translated `init` and `step` compute exactly what
`Crossing.Signal.init` and `Crossing.Signal.step` specify, for every state and
input, without panicking. Spec invariants then transfer to the implementation.

Trusted base: the Lean kernel; Charon and Aeneas faithfully translating the
Rust source (the translation itself is not verified); rustc compiling that
source as Charon read it. Not covered: the generated composite scheduler, the
message routes, and anything outside the `signal` module.
-/

open Aeneas Aeneas.Std Result
open signal_crate

namespace Crossing.Refinement

/-- Abstraction function on states (a bijection between enumerations). -/
def absState : signal.State → Crossing.Signal.State
  | .VehiclesGreen => .vehiclesGreen
  | .VehiclesAmber => .vehiclesAmber
  | .Walk => .walk
  | .Clearance => .clearance

/-- Abstraction function on inputs. -/
def absInput : signal.Input → Crossing.Signal.Input
  | .CrossingRequested => .crossingRequested
  | .TimerExpired => .timerExpired

/-- Abstraction function on outputs. -/
def absOutput : signal.Output → Crossing.Signal.Output
  | .WalkStarted => .walkStarted

/-- Abstraction of a transition result. -/
def absResult (r : signal.State × Option signal.Output) :
    Crossing.Signal.State × Option Crossing.Signal.Output :=
  (absState r.1, r.2.map absOutput)

/-- The implementation's initial state is the specified one. -/
theorem init_refines : ∃ s, signal.init = ok s ∧ absState s = Crossing.Signal.init := by
  exact ⟨.VehiclesGreen, by simp [signal.init, signal.imp.init], rfl⟩

/-- Every implementation step succeeds and agrees with the specification. -/
theorem step_refines (s : signal.State) (i : signal.Input) :
    ∃ r, signal.step s i = ok r ∧ absResult r = Crossing.Signal.step (absState s) (absInput i) := by
  cases s <;> cases i <;> exact ⟨_, rfl, rfl⟩

/-- A spec invariant, transferred to the implementation through `step_refines`:
the implementation enters the walk phase only from amber (or by staying in walk). -/
theorem impl_walk_entered_from_amber (s : signal.State) (i : signal.Input) (r : signal.State × Option signal.Output)
    (h : signal.step s i = ok r) (walk : absState r.1 = .walk) :
    absState s = .vehiclesAmber ∨ absState s = .walk := by
  obtain ⟨r', h', agree⟩ := step_refines s i
  rw [h] at h'
  cases h'
  have : (Crossing.Signal.step (absState s) (absInput i)).1 = .walk := by
    rw [← agree]; exact walk
  exact Crossing.Signal.walk_entered_from_amber _ _ this

end Crossing.Refinement

-- Evidence: the axioms each result depends on (must not include `sorryAx`).
#print axioms Crossing.Refinement.init_refines
#print axioms Crossing.Refinement.step_refines
#print axioms Crossing.Refinement.impl_walk_entered_from_amber
