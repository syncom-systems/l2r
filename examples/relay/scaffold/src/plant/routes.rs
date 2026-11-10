//! Message and port translations for Plant (`plant`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.
//! An edge translation may return `None`: the message then means nothing to
//! the receiver. A port translation is total.

use crate::types::relay;

/// Implements `Relay.Routes.outToReport` (edge `out_to_report`).
pub(crate) fn out_to_report(message: relay::ports::Report) -> Option<relay::ports::Report> {
    Some(message)
}

/// Implements `Relay.Routes.zoneToB` (edge `zone_to_b`).
pub(crate) fn zone_to_b(message: relay::Token) -> Option<relay::Token> {
    Some(message)
}

/// Implements `Relay.Routes.zoneToTap` (edge `zone_to_tap`).
pub(crate) fn zone_to_tap(message: relay::Token) -> Option<relay::Token> {
    if message.value.is_multiple_of(2) {
        Some(message)
    } else {
        None
    }
}

/// Implements `Relay.Routes.zoneToUrgent` (edge `zone_to_urgent`).
pub(crate) fn zone_to_urgent(message: relay::Token) -> Option<relay::Token> {
    if message.value > 1000 {
        Some(message)
    } else {
        None
    }
}

/// Implements `Relay.Ports.plantToZone` (port `poke`).
pub(crate) fn port_poke(message: relay::ports::Poke) -> relay::ports::Poke {
    message
}

#[cfg(test)]
mod tests {
    use super::super::{Progress, Stopped, System};
    use crate::types::relay::Token;
    use crate::types::relay::ports::{Poke, Report};
    use crate::types::relay::sink;

    fn poke(system: &mut System, n: u16) {
        system.send_poke(Poke::Poke(n)).unwrap();
    }

    #[test]
    fn a_small_odd_value_reaches_the_sink_once_and_not_the_tap() {
        let mut system = System::new();
        poke(&mut system, 7);
        assert_eq!(system.run(10), Ok(2));
        let report = Report {
            sum: 7,
            urgent: false,
        };
        assert_eq!(system.take_report(), Some(report));
        assert_eq!(system.take_tap(), None);
    }

    #[test]
    fn a_large_even_value_reaches_the_sink_twice_in_order_and_the_tap() {
        let mut system = System::new();
        poke(&mut system, 2000);
        assert_eq!(system.step_once(), Progress::Delivered);
        // One delivery put two messages in the sink's mailbox, in routing order.
        assert_eq!(system.sink_inbox().get(0), Some(sink::Input::Token(2000)));
        assert_eq!(system.sink_inbox().get(1), Some(sink::Input::Urgent(2000)));
        assert_eq!(system.take_tap(), Some(Token { value: 2000 }));
        assert_eq!(system.run(10), Ok(2));
        let first = Report {
            sum: 2000,
            urgent: false,
        };
        let second = Report {
            sum: 4000,
            urgent: true,
        };
        assert_eq!(system.take_report(), Some(first));
        assert_eq!(system.take_report(), Some(second));
    }

    #[test]
    fn a_delivery_needs_room_for_everything_it_sends_or_nothing_happens() {
        let mut system = System::new();
        poke(&mut system, 7);
        assert_eq!(system.step_once(), Progress::Delivered);
        // The sink holds one message; a large value needs room for two.
        poke(&mut system, 2000);
        let before = system;
        // Round-robin serves the sink first; then the source's delivery fits.
        assert_eq!(system.step_once(), Progress::Delivered);
        assert_eq!(system.source_inbox().len(), before.source_inbox().len());
        assert_eq!(system.step_once(), Progress::Delivered);
        assert_eq!(system.sink_inbox().len(), 2);
    }

    #[test]
    fn a_full_tap_blocks_the_source_and_nothing_is_dropped() {
        let mut system = System::new();
        poke(&mut system, 2);
        assert!(system.run(10).is_ok());
        poke(&mut system, 4);
        // The tap still holds 2: the source cannot deliver 4.
        assert_eq!(system.run(10), Err(Stopped::Blocked { steps: 0 }));
        assert_eq!(system.source_inbox().len(), 1);
        assert_eq!(system.take_tap(), Some(Token { value: 2 }));
        assert!(system.run(10).is_ok());
        assert_eq!(system.take_tap(), Some(Token { value: 4 }));
    }

    #[test]
    fn the_sum_wraps_like_the_lean_specification() {
        let mut system = System::new();
        poke(&mut system, 65_535);
        assert!(system.run(10).is_ok());
        let _ = system.take_report();
        let _ = system.take_report();
        let _ = system.take_tap();
        poke(&mut system, 3);
        assert!(system.run(10).is_ok());
        // 65535 arrived twice (token and urgent): 65535 + 65535 + 3 = 1 (mod 2^16).
        assert_eq!(system.sink_state().sum, 1);
    }
}
