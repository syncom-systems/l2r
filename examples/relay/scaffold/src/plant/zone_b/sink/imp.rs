//! Implementation of Sink (`sink`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or
//! public items, and report contradictions with the Lean specification instead
//! of working around them.

use super::{Input, Output, State};

/// Implements `Relay.Sink.init`.
pub(super) fn init() -> State {
    State {
        sum: 0,
        urgent: false,
    }
}

/// Implements `Relay.Sink.step`. The sum wraps, as Lean's `UInt16` addition does.
pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {
    let (value, urgent) = match input {
        Input::Token(value) => (value, state.urgent),
        Input::Urgent(value) => (value, true),
    };
    let sum = state.sum.wrapping_add(value);
    (State { sum, urgent }, Some(Output::Total(sum, urgent)))
}
