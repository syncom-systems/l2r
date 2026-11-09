/-!
# High-level alarm

Raises once when the level becomes high and stays raised until an operator
acknowledges it, counting how many times it was raised.
-/
namespace Tank.Alarm

structure State where
  active : Bool
  /-- Times the alarm was raised; stays at 255. -/
  count : UInt8
  deriving DecidableEq, Repr

inductive Input where
  | high
  | cleared
  | acknowledge
  deriving DecidableEq, Repr

inductive Output where
  | raised (count : UInt8)
  | silenced
  deriving DecidableEq, Repr

def init : State := ⟨false, 0⟩

/-- `n + 1`, staying at the largest value instead of wrapping. -/
def bump (n : UInt8) : UInt8 := if n = 255 then 255 else n + 1

def step (s : State) : Input → State × Option Output
  | .high =>
    if s.active then (s, none)
    else (⟨true, bump s.count⟩, some (.raised (bump s.count)))
  | .cleared => (s, none)
  | .acknowledge =>
    if s.active then (⟨false, s.count⟩, some .silenced) else (s, none)

/-- Only an acknowledgement silences an active alarm. -/
theorem stays_active_until_acknowledged (s : State) (i : Input) :
    s.active = true → i ≠ .acknowledge → (step s i).1.active = true := by
  intro h hi
  cases i with
  | high => simp [step, h]
  | cleared => simpa [step] using h
  | acknowledge => exact absurd rfl hi

/-- The count never decreases. -/
theorem count_never_decreases (s : State) (i : Input) : s.count ≤ (step s i).1.count := by
  have hb : s.count ≤ bump s.count := by
    unfold bump
    split
    · rename_i h; rw [h]; exact UInt8.le_refl _
    · rename_i h
      rw [UInt8.le_iff_toNat_le]
      have h255 : s.count.toNat ≠ 255 := fun e => h (UInt8.toNat_inj.mp e)
      have hlt : s.count.toNat < 256 := UInt8.toNat_lt s.count
      rw [UInt8.toNat_add]
      simp
      omega
  cases i with
  | high =>
    simp only [step]
    split
    · exact UInt8.le_refl _
    · exact hb
  | cleared => exact UInt8.le_refl _
  | acknowledge =>
    simp only [step]
    split <;> exact UInt8.le_refl _

end Tank.Alarm
