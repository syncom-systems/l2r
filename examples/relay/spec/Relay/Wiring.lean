import Relay.Machines

/-!
# Relay: ports and routes
-/
namespace Relay

namespace Ports

/-- The plant's only input. -/
inductive Poke where
  | poke (n : UInt16)
  deriving DecidableEq, Repr

/-- What the plant reports. -/
structure Report where
  sum : UInt16
  urgent : Bool
  deriving DecidableEq, Repr

def plantToZone : Poke → Poke := fun p => p

def zoneToCell : Poke → Poke := fun p => p

def cellToSource : Poke → Source.Input
  | .poke n => .poke n

def tokenToSink : Token → Sink.Input := fun t => .token t.value

def urgentToSink : Token → Sink.Input := fun t => .urgent t.value

end Ports

namespace Routes

def sourceToCell : Source.Output → Option Token
  | .emitted n => some ⟨n⟩

def cellToZone : Token → Option Token := fun t => some t

def zoneToB : Token → Option Token := fun t => some t

/-- Large values also travel the urgent route: the sink then receives two messages. -/
def zoneToUrgent : Token → Option Token := fun t => if t.value > 1000 then some t else none

/-- Even values are also copied to the tap. -/
def zoneToTap : Token → Option Token := fun t => if t.value % 2 = 0 then some t else none

def sinkToOut : Sink.Output → Option Ports.Report
  | .total s u => some ⟨s, u⟩

def outToReport : Ports.Report → Option Ports.Report := fun r => some r

end Routes

end Relay
