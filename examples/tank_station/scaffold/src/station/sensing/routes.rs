//! Message and port translations for Level sensing (`sensing`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Each function must agree with the Lean translation it names.
//! An edge translation may return `None`: the message then means nothing to
//! the receiver. A port translation is total.

use crate::types::tank;

/// Implements `Tank.Routes.filterToLevel` (edge `filter_to_level`).
pub(crate) fn filter_to_level(message: tank::filter::Output) -> Option<tank::Level> {
    match message {
        tank::filter::Output::LevelChanged(level) => Some(level),
    }
}

/// Implements `Tank.Ports.readingToFilter` (port `reading`).
pub(crate) fn port_reading(message: tank::ports::Sample) -> tank::filter::Input {
    tank::filter::Input::Sample(message.raw)
}
