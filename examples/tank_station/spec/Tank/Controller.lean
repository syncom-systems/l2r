import Tank.Common

/-!
# Valve controller

Decides the inlet valve from the confirmed level and the operator's commands.
The interlock is unconditional: while the level is high the valve is closed, in
every mode and whatever the operator asks.
-/
namespace Tank.Controller

inductive Mode where
  | auto
  | manual
  deriving DecidableEq, Repr

inductive Valve where
  | closed
  | opened
  deriving DecidableEq, Repr

inductive Command where
  | setAuto
  | forceOpen
  | forceClose
  deriving DecidableEq, Repr

structure State where
  mode : Mode
  valve : Valve
  level : Level
  deriving DecidableEq, Repr

inductive Input where
  | level (level : Level)
  | command (command : Command)
  deriving DecidableEq, Repr

/-- What the controller reports after every input. -/
structure Output where
  valve : Valve
  level : Level
  /-- The valve position differs from the one before this input. -/
  changed : Bool
  deriving DecidableEq, Repr

def init : State := ⟨.auto, .closed, .normal⟩

/-- The valve position for a mode, the current position and a level. -/
def position (mode : Mode) (valve : Valve) (level : Level) : Valve :=
  match level with
  | .high => .closed
  | .low =>
    match mode with
    | .auto => .opened
    | .manual => valve
  | .normal => valve

/-- The state and report after moving to `mode` with the operator wanting `wanted`. -/
def settle (s : State) (mode : Mode) (wanted : Valve) (level : Level) : State × Option Output :=
  let v := position mode wanted level
  (⟨mode, v, level⟩, some ⟨v, level, v != s.valve⟩)

def step (s : State) : Input → State × Option Output
  | .level l => settle s s.mode s.valve l
  | .command .setAuto => settle s .auto s.valve s.level
  | .command .forceOpen => settle s .manual .opened s.level
  | .command .forceClose => settle s .manual .closed s.level

/-- **Interlock.** After every input, a high level means a closed valve. -/
theorem high_level_closes_valve (s : State) (i : Input) :
    (step s i).1.level = .high → (step s i).1.valve = .closed := by
  cases i with
  | level l => intro h; simp only [step, settle] at h ⊢; subst h; rfl
  | command c => cases c <;> (intro h; simp only [step, settle] at h ⊢; rw [h]; rfl)

/-- Every input is answered by a report of the resulting valve position and level. -/
theorem always_reports (s : State) (i : Input) :
    (step s i).2 = some ⟨(step s i).1.valve, (step s i).1.level, (step s i).1.valve != s.valve⟩ := by
  cases i with
  | level l => rfl
  | command c => cases c <;> rfl

end Tank.Controller
