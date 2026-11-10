//! Message and port translations for Zone A (`zone_a`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.
//! An edge translation may return `None`: the message then means nothing to
//! the receiver. A port translation is total.

use crate::types::relay;

/// Implements `Relay.Routes.cellToZone` (edge `cell_to_zone`).
pub(crate) fn cell_to_zone(message: relay::Token) -> Option<relay::Token> {
    Some(message)
}

/// Implements `Relay.Ports.zoneToCell` (port `zone_a_in`).
pub(crate) fn port_zone_a_in(message: relay::ports::Poke) -> relay::ports::Poke {
    message
}
