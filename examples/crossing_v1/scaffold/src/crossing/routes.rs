//! Message and port translations for Crossing controller (`crossing`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.
//! Port translations are total: a port message always becomes an input.

use super::{ButtonEvent, TimerEvent, request_latch, signal};

/// Implements `CrossingV1.Routes.latchToSignal` (edge `latch_to_signal`). `None` means the output means nothing to `signal`.
pub(super) fn latch_to_signal(output: request_latch::Output) -> Option<signal::Input> {
    match output {
        request_latch::Output::Requested => Some(signal::Input::CrossingRequested),
    }
}

/// Implements `CrossingV1.Routes.signalToLatch` (edge `signal_to_latch`). `None` means the output means nothing to `request_latch`.
pub(super) fn signal_to_latch(output: signal::Output) -> Option<request_latch::Input> {
    match output {
        signal::Output::WalkStarted => Some(request_latch::Input::Acknowledged),
    }
}

/// Implements `CrossingV1.Ports.buttonToLatch` (port `button`).
pub(super) fn port_button(message: ButtonEvent) -> request_latch::Input {
    match message {
        ButtonEvent::Pressed => request_latch::Input::ButtonPressed,
    }
}

/// Implements `CrossingV1.Ports.timerToSignal` (port `timer`).
pub(super) fn port_timer(message: TimerEvent) -> signal::Input {
    match message {
        TimerEvent::Expired => signal::Input::TimerExpired,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        ButtonEvent, Progress, Rejected, System, TimerEvent, request_latch, signal,
    };

    #[test]
    fn a_request_reaches_the_signal_and_is_acknowledged() {
        let mut system = System::new();
        assert_eq!(system.send_button(ButtonEvent::Pressed), Ok(()));
        assert_eq!(system.run(10), Ok(2));
        assert_eq!(system.signal(), signal::State::VehiclesAmber);
        assert_eq!(system.request_latch(), request_latch::State::Pending);
        assert_eq!(system.send_timer(TimerEvent::Expired), Ok(()));
        assert!(system.run(10).is_ok());
        assert_eq!(system.signal(), signal::State::Walk);
        assert_eq!(system.request_latch(), request_latch::State::Idle);
    }

    #[test]
    fn a_full_mailbox_rejects_the_port_send_and_keeps_what_it_holds() {
        let mut system = System::new();
        assert_eq!(system.send_timer(TimerEvent::Expired), Ok(()));
        assert_eq!(system.send_timer(TimerEvent::Expired), Ok(()));
        assert_eq!(system.send_timer(TimerEvent::Expired), Err(Rejected));
        assert_eq!(system.signal_inbox().len(), 2);
        assert_eq!(system.step_once(), Progress::Delivered);
        assert_eq!(system.send_timer(TimerEvent::Expired), Ok(()));
    }

    #[test]
    fn a_request_arriving_during_the_walk_phase_is_served_not_stranded() {
        let mut system = System::new();
        let tick = |system: &mut System| {
            system.send_timer(TimerEvent::Expired).unwrap();
            system.run(20).unwrap();
        };
        system.send_button(ButtonEvent::Pressed).unwrap();
        system.run(20).unwrap();
        tick(&mut system); // amber -> walk, acknowledged
        assert_eq!(system.request_latch(), request_latch::State::Idle);
        // A second pedestrian presses during the walk phase.
        system.send_button(ButtonEvent::Pressed).unwrap();
        system.run(20).unwrap();
        assert_eq!(system.signal(), signal::State::WalkQueued);
        assert_eq!(system.request_latch(), request_latch::State::Pending);
        tick(&mut system); // walk -> clearance (queued)
        tick(&mut system); // clearance -> amber
        tick(&mut system); // amber -> walk, acknowledged
        assert_eq!(system.request_latch(), request_latch::State::Idle);
        assert_eq!(system.signal(), signal::State::Walk);
    }
}
