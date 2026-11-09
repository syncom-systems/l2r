import L2RDerived.tank_station

/-!
# System properties of the tank station

Proved about the flat network that the architecture model approves
(`L2RDerived.tank_station`), over **every** permitted interleaving and for
**every** environment: whatever samples and operator commands arrive, in
whatever order deliveries happen, and whenever (or whether) the environment
empties the outboxes.

## The interlock, as a system property

`Tank.Controller.high_level_closes_valve` is a theorem about one transition of
one machine. Here it becomes a property of the composed system: in every
reachable configuration, a controller that knows the level is high holds the
valve closed. The proof is the standard shape for a `v2` system property and
uses no enumeration of states (the machines carry data):

1. the initial configuration satisfies the invariant;
2. a delivery to the controller preserves it, by the machine's own theorem;
3. a delivery to any other child, a port send, and the environment taking from
   an outbox do not touch the controller's state.

`scheduler_interlock` then transfers it to every run of the Lean model of the
generated scheduler. That the generated Rust behaves like that model is
established by conformance testing, not by this file.
-/

open L2R.Async1
open L2R.Async (update update_same update_other)

namespace TankStationProperties

abbrev N : Network := L2RDerived.«tank_station».«station».network
abbrev machines : Nat := L2RDerived.«tank_station».«station».machines

/-- The controller's state: child 1 of the flat network. -/
def controller (k : Config N) : Tank.Controller.State := k.state ⟨1, by decide⟩

/-- A controller that knows the level is high holds the valve closed. -/
def Interlock (k : Config N) : Prop :=
  (controller k).level = .high → (controller k).valve = .closed

theorem interlock_initial : Interlock (initial N) := by
  intro h
  cases h

/-- A delivery to a child other than the controller leaves the controller alone. -/
theorem controller_untouched {k k' : Config N} {c : Fin N.size} (hc : c ≠ ⟨1, by decide⟩)
    (h : deliver N k c = some k') : controller k' = controller k := by
  obtain ⟨i, rest, _, hstate, _, _⟩ := deliver_some N h
  unfold controller
  rw [hstate]
  exact update_other _ _ (fun e => hc e.symm)

/-- A delivery to the controller is one transition of `Tank.Controller.step`. -/
theorem controller_stepped {k k' : Config N} (h : deliver N k ⟨1, by decide⟩ = some k') :
    ∃ i : Tank.Controller.Input, controller k' = (Tank.Controller.step (controller k) i).1 := by
  obtain ⟨i, rest, _, hstate, _, _⟩ := deliver_some N h
  refine ⟨i, ?_⟩
  unfold controller
  rw [hstate]
  exact update_same _ _ _

/-- Every permitted step preserves the interlock. -/
theorem interlock_step (E : Env N) {k k' : Config N} (hk : Interlock k) (hs : Step N E k k') :
    Interlock k' := by
  obtain ⟨l, hl⟩ := hs
  cases hl with
  | deliver c hd =>
    by_cases hc : c = ⟨1, by decide⟩
    · subst hc
      obtain ⟨i, hi⟩ := controller_stepped hd
      unfold Interlock
      rw [hi]
      exact Tank.Controller.high_level_closes_valve _ _
    · unfold Interlock
      rw [controller_untouched hc hd]
      exact hk
  | send p m _ _ => exact hk

/-- **The interlock holds in every reachable configuration, for every environment.** -/
theorem interlock (E : Env N) : ∀ k, Reachable N E k → Interlock k :=
  reachable_invariant N E Interlock interlock_initial
    (fun _ _ _ hk hs => interlock_step E hk hs)

/-- Hence it holds on every run of the generated scheduler's Lean model, whatever
the caller sends and whenever it takes from the outboxes. -/
theorem scheduler_interlock (E : Env N) {s : Ports.Sched N} (h : Ports.Run N machines E s) :
    Interlock s.cfg :=
  Ports.run_invariant N machines E Interlock (interlock E) h

/-- No mailbox and no outbox ever exceeds its capacity. -/
theorem buffers_bounded (E : Env N) {k : Config N} (h : Reachable N E k) : Bounded N k :=
  h.bounded N E

end TankStationProperties
