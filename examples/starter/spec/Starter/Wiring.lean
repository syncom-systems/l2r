import Starter.Monitor

/-!
# Ports and routes

The typed interface between the system and its environment, and what each
approved message edge means.
-/
namespace Starter

namespace Ports

/-- A reading from the outside world. -/
structure Sample where
  value : UInt8
  deriving DecidableEq, Repr

/-- What an operator can ask. -/
inductive Command where
  | reset
  deriving DecidableEq, Repr

/-- What the system tells the outside world. -/
inductive Notice where
  | alert (value : UInt8)
  | cleared
  deriving DecidableEq, Repr

def sampleToMonitor : Sample → Monitor.Input := fun s => .reading s.value

def commandToMonitor : Command → Monitor.Input
  | .reset => .reset

end Ports

namespace Routes

def monitorToNotice : Monitor.Output → Option Ports.Notice
  | .alert v => some (.alert v)
  | .cleared => some .cleared

end Routes

end Starter
