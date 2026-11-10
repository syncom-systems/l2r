import Tank.Common

/-!
# Level filter

Classifies raw level samples and reports a change of classification only after
it has been seen in `confirmAfter` consecutive samples, so that a single noisy
sample does not move the valve.
-/
namespace Tank.Filter

structure State where
  /-- The classification last reported. -/
  reported : Level
  /-- The classification currently being confirmed. -/
  candidate : Level
  /-- Consecutive samples that agreed with `candidate`. -/
  streak : UInt8
  deriving DecidableEq, Repr

inductive Input where
  | sample (raw : UInt8)
  deriving DecidableEq, Repr

inductive Output where
  | levelChanged (level : Level)
  deriving DecidableEq, Repr

/-- Samples below 20 are low, above 200 high. -/
def classify (raw : UInt8) : Level :=
  if raw < 20 then .low else if raw > 200 then .high else .normal

/-- Agreeing samples needed before a change is reported. -/
def confirmAfter : UInt8 := 2

def init : State := ⟨.normal, .normal, 0⟩

/-- `n + 1`, staying at the largest value instead of wrapping. -/
def bump (n : UInt8) : UInt8 := if n = 255 then 255 else n + 1

def step (s : State) : Input → State × Option Output
  | .sample raw =>
    let l := classify raw
    if l = s.reported then
      (⟨s.reported, l, 0⟩, none)
    else if l = s.candidate then
      if bump s.streak ≥ confirmAfter then
        (⟨l, l, 0⟩, some (.levelChanged l))
      else
        (⟨s.reported, l, bump s.streak⟩, none)
    else
      (⟨s.reported, l, 1⟩, none)

/-- A change is reported only for a level that differs from the one reported before. -/
theorem reports_only_changes (s : State) (i : Input) (l : Level) :
    (step s i).2 = some (.levelChanged l) → l ≠ s.reported ∧ (step s i).1.reported = l := by
  cases i with
  | sample raw =>
    simp only [step]
    split
    · simp
    · rename_i h
      split
      · split
        · intro h2
          simp only [Option.some.injEq, Output.levelChanged.injEq] at h2
          subst h2
          exact ⟨h, rfl⟩
        · simp
      · simp

/-- A single deviating sample never changes the reported level. -/
theorem one_sample_is_not_enough (s : State) (i : Input) :
    s.streak = 0 → (step s i).1.reported = s.reported := by
  intro h0
  cases i with
  | sample raw =>
    simp only [step]
    split
    · rfl
    · split
      · have : ¬ bump s.streak ≥ confirmAfter := by rw [h0]; decide
        simp [this]
      · rfl

end Tank.Filter
