/-!
# Crossing signal

Cycles vehicle traffic and pedestrian traffic. Pedestrians may walk only after the
vehicle phase has passed through amber.
-/
namespace Crossing.Signal

inductive State where
  | vehiclesGreen
  | vehiclesAmber
  | walk
  | clearance
  deriving DecidableEq, Repr

inductive Input where
  | crossingRequested
  | timerExpired
  deriving DecidableEq, Repr

inductive Output where
  | walkStarted
  deriving DecidableEq, Repr

def init : State := .vehiclesGreen

def step : State → Input → State × Option Output
  | .vehiclesGreen, .crossingRequested => (.vehiclesAmber, none)
  | .vehiclesAmber, .timerExpired => (.walk, some .walkStarted)
  | .walk, .timerExpired => (.clearance, none)
  | .clearance, .timerExpired => (.vehiclesGreen, none)
  | s, _ => (s, none)

def vehiclesMayMove : State → Bool
  | .vehiclesGreen | .vehiclesAmber => true
  | _ => false

def pedestriansMayWalk : State → Bool
  | .walk => true
  | _ => false

/-- Vehicles and pedestrians are never released at the same time. -/
theorem no_conflicting_release (s : State) :
    ¬ (vehiclesMayMove s = true ∧ pedestriansMayWalk s = true) := by
  cases s <;> simp [vehiclesMayMove, pedestriansMayWalk]

/-- The walk phase is entered only from amber (or by staying in walk). -/
theorem walk_entered_from_amber (s : State) (i : Input) :
    (step s i).1 = .walk → s = .vehiclesAmber ∨ s = .walk := by
  cases s <;> cases i <;> simp [step]

end Crossing.Signal
