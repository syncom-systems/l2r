/-!
# Crossing signal (repaired)

Cycles vehicle traffic and pedestrian traffic. Pedestrians may walk only after the
vehicle phase has passed through amber.

**Repair of the stranded-request defect.** The signal of the original example
ignored a crossing request that arrived outside the green phase, while the latch
waits for an acknowledgement forever. Here a request that arrives during the walk
or clearance phase is *remembered* (`walkQueued`, `clearanceQueued`): when the
vehicle phase would return to green, the signal goes to amber instead and serves
it, acknowledging it when the next walk phase starts. A request that arrives in
amber is already being served. Every request therefore leads to an
acknowledgement once the timer keeps expiring (see
`CrossingV1.Liveness` in the composition proofs; the timer is an assumption about
the environment, stated there).
-/
namespace CrossingV1.Signal

inductive State where
  | vehiclesGreen
  | vehiclesAmber
  | walk
  | walkQueued
  | clearance
  | clearanceQueued
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
  | .vehiclesAmber, .crossingRequested => (.vehiclesAmber, none)
  | .walk, .crossingRequested => (.walkQueued, none)
  | .walkQueued, .crossingRequested => (.walkQueued, none)
  | .clearance, .crossingRequested => (.clearanceQueued, none)
  | .clearanceQueued, .crossingRequested => (.clearanceQueued, none)
  | .vehiclesGreen, .timerExpired => (.vehiclesGreen, none)
  | .vehiclesAmber, .timerExpired => (.walk, some .walkStarted)
  | .walk, .timerExpired => (.clearance, none)
  | .walkQueued, .timerExpired => (.clearanceQueued, none)
  | .clearance, .timerExpired => (.vehiclesGreen, none)
  | .clearanceQueued, .timerExpired => (.vehiclesAmber, none)

def vehiclesMayMove : State → Bool
  | .vehiclesGreen | .vehiclesAmber => true
  | _ => false

def pedestriansMayWalk : State → Bool
  | .walk | .walkQueued => true
  | _ => false

/-- Vehicles and pedestrians are never released at the same time. -/
theorem no_conflicting_release (s : State) :
    ¬ (vehiclesMayMove s = true ∧ pedestriansMayWalk s = true) := by
  cases s <;> simp [vehiclesMayMove, pedestriansMayWalk]

/-- The walk phase is entered only from amber (or by staying in it). -/
theorem walk_entered_from_amber (s : State) (i : Input) :
    pedestriansMayWalk (step s i).1 = true →
      s = .vehiclesAmber ∨ pedestriansMayWalk s = true := by
  cases s <;> cases i <;> simp [step, pedestriansMayWalk]

end CrossingV1.Signal
