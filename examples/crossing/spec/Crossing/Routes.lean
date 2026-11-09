import Crossing.Latch
import Crossing.Signal

/-!
# Message translation between the crossing's machines

These functions give the meaning of each message edge in the architecture model.
The model decides *which* components may exchange messages; Lean decides *what*
a message means to its receiver.
-/
namespace Crossing.Routes

def latchToSignal : Latch.Output → Option Signal.Input
  | .requested => some .crossingRequested

def signalToLatch : Signal.Output → Option Latch.Input
  | .walkStarted => some .acknowledged

end Crossing.Routes
