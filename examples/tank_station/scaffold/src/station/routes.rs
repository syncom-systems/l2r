//! Message and port translations for Filling station (`station`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.
//! An edge translation may return `None`: the message then means nothing to
//! the receiver. A port translation is total.

use crate::types::tank;
use crate::types::tank::controller::Valve;
use crate::types::tank::ports::{AlarmNotice, Operator, Sample, ValveCommand};

/// Implements `Tank.Routes.alarmToNotice` (edge `alarm_to_notice`).
pub(crate) fn alarm_to_notice(message: tank::alarm::Output) -> Option<AlarmNotice> {
    match message {
        tank::alarm::Output::Raised(count) => Some(AlarmNotice {
            active: true,
            count: Some(count),
        }),
        tank::alarm::Output::Silenced => Some(AlarmNotice {
            active: false,
            count: None,
        }),
    }
}

/// Implements `Tank.Routes.controllerToAlarm` (edge `controller_to_alarm`).
pub(crate) fn controller_to_alarm(message: tank::controller::Output) -> Option<tank::alarm::Input> {
    match message.level {
        tank::Level::High => Some(tank::alarm::Input::High),
        tank::Level::Low => Some(tank::alarm::Input::Cleared),
        tank::Level::Normal => Some(tank::alarm::Input::Cleared),
    }
}

/// Implements `Tank.Routes.controllerToValve` (edge `controller_to_valve`).
pub(crate) fn controller_to_valve(message: tank::controller::Output) -> Option<ValveCommand> {
    if message.changed {
        match message.valve {
            Valve::Opened => Some(ValveCommand::Open),
            Valve::Closed => Some(ValveCommand::Close),
        }
    } else {
        None
    }
}

/// Implements `Tank.Routes.levelToController` (edge `level_to_controller`).
pub(crate) fn level_to_controller(message: tank::Level) -> Option<tank::controller::Input> {
    Some(tank::controller::Input::Level(message))
}

/// Implements `Tank.Ports.acknowledgeToAlarm` (port `acknowledge`).
pub(crate) fn port_acknowledge(message: tank::ports::Acknowledge) -> tank::alarm::Input {
    match message {
        tank::ports::Acknowledge::Acknowledge => tank::alarm::Input::Acknowledge,
    }
}

/// Implements `Tank.Ports.operatorToController` (port `operator`).
pub(crate) fn port_operator(message: Operator) -> tank::controller::Input {
    use tank::controller::Command;
    match message {
        Operator::SetAuto => tank::controller::Input::Command(Command::SetAuto),
        Operator::ForceOpen => tank::controller::Input::Command(Command::ForceOpen),
        Operator::ForceClose => tank::controller::Input::Command(Command::ForceClose),
    }
}

/// Implements `Tank.Ports.sampleToSensing` (port `sample`).
pub(crate) fn port_sample(message: Sample) -> Sample {
    message
}

#[cfg(test)]
mod tests {
    use super::super::{Progress, Rejected, Stopped, System};
    use crate::types::tank::Level;
    use crate::types::tank::controller::Valve;
    use crate::types::tank::ports::{Acknowledge, AlarmNotice, Operator, Sample, ValveCommand};

    fn sample(system: &mut System, raw: u8) {
        system.send_sample(Sample { raw }).unwrap();
        system.run(20).unwrap();
    }

    #[test]
    fn a_confirmed_low_level_opens_the_valve_and_tells_the_actuator() {
        let mut system = System::new();
        sample(&mut system, 5);
        assert_eq!(system.controller_state().valve, Valve::Closed);
        assert_eq!(system.take_valve_command(), None);
        sample(&mut system, 5);
        assert_eq!(system.controller_state().level, Level::Low);
        assert_eq!(system.controller_state().valve, Valve::Opened);
        assert_eq!(system.take_valve_command(), Some(ValveCommand::Open));
        assert_eq!(system.take_valve_command(), None);
    }

    #[test]
    fn a_high_level_closes_the_valve_raises_the_alarm_and_resists_the_operator() {
        let mut system = System::new();
        sample(&mut system, 5);
        sample(&mut system, 5);
        assert_eq!(system.take_valve_command(), Some(ValveCommand::Open));
        sample(&mut system, 250);
        sample(&mut system, 250);
        assert_eq!(system.controller_state().valve, Valve::Closed);
        assert_eq!(system.take_valve_command(), Some(ValveCommand::Close));
        let raised = AlarmNotice {
            active: true,
            count: Some(1),
        };
        assert_eq!(system.take_alarm_notice(), Some(raised));
        system.send_operator(Operator::ForceOpen).unwrap();
        system.run(20).unwrap();
        assert_eq!(system.controller_state().valve, Valve::Closed);
        assert_eq!(system.take_valve_command(), None);
        system.send_acknowledge(Acknowledge::Acknowledge).unwrap();
        system.run(20).unwrap();
        let silenced = AlarmNotice {
            active: false,
            count: None,
        };
        assert_eq!(system.take_alarm_notice(), Some(silenced));
    }

    #[test]
    fn a_full_mailbox_rejects_the_send_and_keeps_what_it_holds() {
        let mut system = System::new();
        assert_eq!(system.send_sample(Sample { raw: 1 }), Ok(()));
        assert_eq!(system.send_sample(Sample { raw: 2 }), Ok(()));
        assert_eq!(system.send_sample(Sample { raw: 3 }), Err(Rejected));
        assert_eq!(system.filter_inbox().len(), 2);
        assert_eq!(system.step_once(), Progress::Delivered);
        assert_eq!(system.send_sample(Sample { raw: 3 }), Ok(()));
    }

    #[test]
    fn an_outbox_nobody_empties_blocks_the_sender_and_taking_unblocks_it() {
        let mut system = System::new();
        // Alternate forced positions: every command changes the valve.
        system.send_operator(Operator::ForceOpen).unwrap();
        system.send_operator(Operator::ForceClose).unwrap();
        system.send_operator(Operator::ForceOpen).unwrap();
        // Two commands fit in the outbox; the third delivery has nowhere to put its message.
        assert_eq!(system.run(20), Err(Stopped::Blocked { steps: 4 }));
        assert_eq!(system.valve_command_outbox().len(), 2);
        assert_eq!(system.controller_inbox().len(), 1);
        assert_eq!(system.take_valve_command(), Some(ValveCommand::Open));
        assert!(system.run(20).is_ok());
        assert_eq!(system.take_valve_command(), Some(ValveCommand::Close));
        assert_eq!(system.take_valve_command(), Some(ValveCommand::Open));
        assert_eq!(system.controller_state().valve, Valve::Opened);
    }
}
