//! Message and port translations for Starter (`starter`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.
//! An edge translation may return `None`: the message then means nothing to
//! the receiver. A port translation is total.

use crate::types::starter::monitor;
use crate::types::starter::ports::{Command, Notice, Sample};

/// Implements `Starter.Routes.monitorToNotice` (edge `monitor_to_notice`).
pub(crate) fn monitor_to_notice(message: monitor::Output) -> Option<Notice> {
    match message {
        monitor::Output::Alert(value) => Some(Notice::Alert(value)),
        monitor::Output::Cleared => Some(Notice::Cleared),
    }
}

/// Implements `Starter.Ports.commandToMonitor` (port `command`).
pub(crate) fn port_command(message: Command) -> monitor::Input {
    match message {
        Command::Reset => monitor::Input::Reset,
    }
}

/// Implements `Starter.Ports.sampleToMonitor` (port `sample`).
pub(crate) fn port_sample(message: Sample) -> monitor::Input {
    monitor::Input::Reading(message.value)
}

#[cfg(test)]
mod tests {
    use super::super::{Progress, Rejected, Stopped, System};
    use crate::types::starter::ports::{Command, Notice, Sample};

    #[test]
    fn a_reading_above_the_limit_raises_an_alert_and_a_reset_clears_the_count() {
        let mut system = System::new();
        system.send_sample(Sample { value: 20 }).unwrap();
        system.send_sample(Sample { value: 120 }).unwrap();
        assert_eq!(system.run(10), Ok(2));
        assert_eq!(system.monitor_state().alerts, 1);
        assert_eq!(system.take_notice(), Some(Notice::Alert(120)));
        assert_eq!(system.take_notice(), None);
        system.send_command(Command::Reset).unwrap();
        assert_eq!(system.run(10), Ok(1));
        assert_eq!(system.monitor_state().alerts, 0);
        assert_eq!(system.take_notice(), Some(Notice::Cleared));
    }

    #[test]
    fn a_full_mailbox_rejects_and_a_full_outbox_blocks_without_losing_anything() {
        let mut system = System::new();
        for value in [101, 102, 103, 104] {
            assert_eq!(system.send_sample(Sample { value }), Ok(()));
        }
        assert_eq!(system.send_sample(Sample { value: 105 }), Err(Rejected));
        assert_eq!(system.run(10), Ok(4));
        // The outbox now holds four alerts; a fifth has nowhere to go.
        system.send_sample(Sample { value: 105 }).unwrap();
        assert_eq!(system.run(10), Err(Stopped::Blocked { steps: 0 }));
        assert_eq!(system.take_notice(), Some(Notice::Alert(101)));
        assert_eq!(system.step_once(), Progress::Delivered);
        for value in [102, 103, 104, 105] {
            assert_eq!(system.take_notice(), Some(Notice::Alert(value)));
        }
    }
}
