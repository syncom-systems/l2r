/-!
# Monitor

The starting point `l2r init` gives a new project: one machine that watches
readings, raises an alert for each reading above a limit, and counts the alerts
until it is reset. Replace it with your own machines.

Lean owns the behaviour. The architecture model refers to these declarations by
name; the Rust implementation must compute exactly what `step` says.
-/
namespace Starter.Monitor

structure State where
  /-- The most recent reading. -/
  last : UInt8
  /-- Alerts raised since the last reset; stays at 255. -/
  alerts : UInt8
  deriving DecidableEq, Repr

inductive Input where
  | reading (value : UInt8)
  | reset
  deriving DecidableEq, Repr

inductive Output where
  | alert (value : UInt8)
  | cleared
  deriving DecidableEq, Repr

/-- Readings above this value raise an alert. -/
def limit : UInt8 := 100

def init : State := ⟨0, 0⟩

/-- `n + 1`, staying at the largest value instead of wrapping. -/
def bump (n : UInt8) : UInt8 := if n = 255 then 255 else n + 1

def step (s : State) : Input → State × Option Output
  | .reading v =>
    if v > limit then (⟨v, bump s.alerts⟩, some (.alert v)) else (⟨v, s.alerts⟩, none)
  | .reset => (⟨s.last, 0⟩, some .cleared)

/-- An alert is raised only for a reading above the limit, and it carries that reading. -/
theorem alert_only_above_limit (s : State) (v w : UInt8) :
    (step s (.reading v)).2 = some (.alert w) → w = v ∧ v > limit := by
  simp only [step]
  split
  · rename_i h
    intro hw
    simp only [Option.some.injEq, Output.alert.injEq] at hw
    exact ⟨hw.symm, h⟩
  · simp

/-- A reset always clears the count. -/
theorem reset_clears (s : State) : (step s .reset).1.alerts = 0 := rfl

end Starter.Monitor
