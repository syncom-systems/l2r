//! Implementation of Source (`source`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or
//! public items, and report contradictions with the Lean specification instead
//! of working around them.

use super::{Input, Output, State};

/// Implements `Relay.Source.init`.
pub(super) fn init() -> State {
    State { last: 0 }
}

/// Implements `Relay.Source.step`.
pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {
    let Input::Poke(n) = input;
    let output = if n == state.last {
        None
    } else {
        Some(Output::Emitted(n))
    };
    (State { last: n }, output)
}
