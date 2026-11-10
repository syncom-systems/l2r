//! Message and port translations for Cell (`cell`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.
//! An edge translation may return `None`: the message then means nothing to
//! the receiver. A port translation is total.

use crate::types::relay;

/// Implements `Relay.Routes.sourceToCell` (edge `source_to_cell`).
pub(crate) fn source_to_cell(message: relay::source::Output) -> Option<relay::Token> {
    let relay::source::Output::Emitted(value) = message;
    Some(relay::Token { value })
}

/// Implements `Relay.Ports.cellToSource` (port `cell_in`).
pub(crate) fn port_cell_in(message: relay::ports::Poke) -> relay::source::Input {
    let relay::ports::Poke::Poke(n) = message;
    relay::source::Input::Poke(n)
}
