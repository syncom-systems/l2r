//! Message and port translations for Zone B (`zone_b`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.
//! An edge translation may return `None`: the message then means nothing to
//! the receiver. A port translation is total.

use crate::types::relay;

/// Implements `Relay.Routes.sinkToOut` (edge `sink_to_out`).
pub(crate) fn sink_to_out(message: relay::sink::Output) -> Option<relay::ports::Report> {
    let relay::sink::Output::Total(sum, urgent) = message;
    Some(relay::ports::Report { sum, urgent })
}

/// Implements `Relay.Ports.tokenToSink` (port `zone_b_in`).
pub(crate) fn port_zone_b_in(message: relay::Token) -> relay::sink::Input {
    relay::sink::Input::Token(message.value)
}

/// Implements `Relay.Ports.urgentToSink` (port `zone_b_urgent`).
pub(crate) fn port_zone_b_urgent(message: relay::Token) -> relay::sink::Input {
    relay::sink::Input::Urgent(message.value)
}
