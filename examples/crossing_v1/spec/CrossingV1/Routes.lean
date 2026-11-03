import CrossingV1.Latch
import CrossingV1.Signal

/-!
# Message translation between the crossing's machines

These functions give the meaning of each message edge in the architecture model.
-/
namespace CrossingV1.Routes

def latchToSignal : Latch.Output → Option Signal.Input
  | .requested => some .crossingRequested

def signalToLatch : Signal.Output → Option Latch.Input
  | .walkStarted => some .acknowledged

end CrossingV1.Routes
