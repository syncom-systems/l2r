import Tank.Common
import Tank.Controller
import Tank.Alarm
import Tank.Filter

/-!
# Ports: the typed interface between the station and its environment
-/
namespace Tank.Ports

/-- A raw level sample from the sensor driver. -/
structure Sample where
  raw : UInt8
  deriving DecidableEq, Repr

/-- An operator command for the valve controller. -/
inductive Operator where
  | setAuto
  | forceOpen
  | forceClose
  deriving DecidableEq, Repr

/-- The operator acknowledges the alarm. -/
inductive Acknowledge where
  | acknowledge
  deriving DecidableEq, Repr

/-- A command for the valve actuator. -/
inductive ValveCommand where
  | open
  | close
  deriving DecidableEq, Repr

/-- What the alarm annunciator is told; `count` is present when the alarm is raised. -/
structure AlarmNotice where
  active : Bool
  count : Option UInt8
  deriving DecidableEq, Repr

/-- The station's `sample` port hands the sample to the sensing subsystem unchanged. -/
def sampleToSensing : Sample → Sample := fun s => s

/-- The sensing subsystem's `reading` port feeds the filter. -/
def readingToFilter : Sample → Filter.Input := fun s => .sample s.raw

def operatorToController : Operator → Controller.Input
  | .setAuto => .command .setAuto
  | .forceOpen => .command .forceOpen
  | .forceClose => .command .forceClose

def acknowledgeToAlarm : Acknowledge → Alarm.Input
  | .acknowledge => .acknowledge

end Tank.Ports
