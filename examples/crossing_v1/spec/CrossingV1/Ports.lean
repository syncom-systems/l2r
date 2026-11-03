import CrossingV1.Latch
import CrossingV1.Signal

/-!
# The crossing controller's external interface

The environment can do exactly two things to the controller: a pedestrian presses
the button, and the timer expires. Each is a typed port with its own message type.
There is no port through which the environment could hand the signal a crossing
request or the latch an acknowledgement: that is a statement about the
*structure* of the interface and holds by the types alone. *When* these events
happen is a different question, answered by an environment assumption in the
system proofs, not here.
-/
namespace CrossingV1.Ports

/-- A pedestrian pressed the button. -/
inductive ButtonEvent where
  | pressed
  deriving DecidableEq, Repr

/-- The signal's timer expired. -/
inductive TimerEvent where
  | expired
  deriving DecidableEq, Repr

def buttonToLatch : ButtonEvent → Latch.Input
  | .pressed => .buttonPressed

def timerToSignal : TimerEvent → Signal.Input
  | .expired => .timerExpired

end CrossingV1.Ports
