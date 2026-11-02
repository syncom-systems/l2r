//! Message translations for Crossing controller (`crossing`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.

use super::{request_latch, signal};

/// Implements `Crossing.Routes.latchToSignal` (edge `latch_to_signal`).
pub(super) fn latch_to_signal(output: request_latch::Output) -> Option<signal::Input> {
    match output {
        request_latch::Output::Requested => Some(signal::Input::CrossingRequested),
    }
}

/// Implements `Crossing.Routes.signalToLatch` (edge `signal_to_latch`).
pub(super) fn signal_to_latch(output: signal::Output) -> Option<request_latch::Input> {
    match output {
        signal::Output::WalkStarted => Some(request_latch::Input::Acknowledged),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Envelope, System, request_latch, signal};

    #[test]
    fn a_request_reaches_the_signal_and_is_acknowledged() {
        let mut system = System::new();
        system.send(Envelope::RequestLatch(request_latch::Input::ButtonPressed));
        system.send(Envelope::RequestLatch(request_latch::Input::ButtonPressed));
        assert!(system.run(10).is_ok());
        assert_eq!(system.signal(), signal::State::VehiclesAmber);
        assert_eq!(system.request_latch(), request_latch::State::Pending);

        system.send(Envelope::Signal(signal::Input::TimerExpired));
        assert!(system.run(10).is_ok());
        assert_eq!(system.signal(), signal::State::Walk);
        assert_eq!(system.request_latch(), request_latch::State::Idle);
    }
}
