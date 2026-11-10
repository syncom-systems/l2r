import Tank.Ports

/-!
# Routes: what each approved message edge means
-/
namespace Tank.Routes

/-- Inside sensing: a confirmed change leaves through the `level_changed` port. -/
def filterToLevel : Filter.Output → Option Level
  | .levelChanged l => some l

/-- The station forwards a confirmed level to the controller. -/
def levelToController : Level → Option Controller.Input
  | l => some (.level l)

/-- The actuator is told only when the valve position changed. -/
def controllerToValve : Controller.Output → Option Ports.ValveCommand
  | ⟨.opened, _, true⟩ => some .open
  | ⟨.closed, _, true⟩ => some .close
  | ⟨_, _, false⟩ => none

/-- The alarm hears every report: high, or cleared. -/
def controllerToAlarm : Controller.Output → Option Alarm.Input
  | ⟨_, .high, _⟩ => some .high
  | ⟨_, _, _⟩ => some .cleared

def alarmToNotice : Alarm.Output → Option Ports.AlarmNotice
  | .raised n => some ⟨true, some n⟩
  | .silenced => some ⟨false, none⟩

end Tank.Routes
