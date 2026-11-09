/-!
# Relay: a deliberately awkward hierarchy

This specification exists to exercise the generator, not to model anything: a
machine three composites deep, a chain of input ports, one output that reaches
the same receiver by two routes, and integer arithmetic that wraps.
-/
namespace Relay

/-- What travels between the zones. -/
structure Token where
  value : UInt16
  deriving DecidableEq, Repr

namespace Source

structure State where
  last : UInt16
  deriving DecidableEq, Repr

inductive Input where
  | poke (n : UInt16)
  deriving DecidableEq, Repr

inductive Output where
  | emitted (n : UInt16)
  deriving DecidableEq, Repr

def init : State := ⟨0⟩

/-- Remembers the last number and emits it when it differs from the one before. -/
def step (s : State) : Input → State × Option Output
  | .poke n => (⟨n⟩, if n = s.last then none else some (.emitted n))

/-- The source never emits the number it already holds. -/
theorem emits_only_news (s : State) (n : UInt16) :
    (step s (.poke n)).2 = some (.emitted n) → n ≠ s.last := by
  simp only [step]
  split
  · simp
  · intro _; assumption

end Source

namespace Sink

structure State where
  sum : UInt16
  urgent : Bool
  deriving DecidableEq, Repr

inductive Input where
  | token (value : UInt16)
  | urgent (value : UInt16)
  deriving DecidableEq, Repr

inductive Output where
  | total (sum : UInt16) (urgent : Bool)
  deriving DecidableEq, Repr

def init : State := ⟨0, false⟩

/-- Adds every value it receives. The sum **wraps** at 2^16, as `UInt16` does. -/
def step (s : State) : Input → State × Option Output
  | .token v => (⟨s.sum + v, s.urgent⟩, some (.total (s.sum + v) s.urgent))
  | .urgent v => (⟨s.sum + v, true⟩, some (.total (s.sum + v) true))

/-- Once urgent, always urgent. -/
theorem urgent_is_sticky (s : State) (i : Input) :
    s.urgent = true → (step s i).1.urgent = true := by
  intro h
  cases i <;> simp [step, h]

end Sink

end Relay
