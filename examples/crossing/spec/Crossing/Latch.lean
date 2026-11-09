/-!
# Request latch

Remembers that a pedestrian asked to cross until the signal acknowledges the request.
Repeated button presses while a request is pending are coalesced into one request.
-/
namespace Crossing.Latch

inductive State where
  | idle
  | pending
  deriving DecidableEq, Repr

inductive Input where
  | buttonPressed
  | acknowledged
  deriving DecidableEq, Repr

inductive Output where
  | requested
  deriving DecidableEq, Repr

def init : State := .idle

def step : State → Input → State × Option Output
  | .idle, .buttonPressed => (.pending, some .requested)
  | .pending, .buttonPressed => (.pending, none)
  | _, .acknowledged => (.idle, none)

/-- A press while a request is pending never emits a second request. -/
theorem pending_press_is_silent : step .pending .buttonPressed = (.pending, none) := rfl

/-- Every request emitted by the latch leaves it pending. -/
theorem request_implies_pending (s : State) (i : Input) :
    (step s i).2 = some .requested → (step s i).1 = .pending := by
  cases s <;> cases i <;> simp [step]

end Crossing.Latch
