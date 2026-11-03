//! Implementation of Request latch (`request_latch`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or
//! public items, and report contradictions with the Lean specification instead
//! of working around them.
//!
//! Size budget: at most 60 lines (checked by `l2r check`).

use super::{Input, Output, State};

/// Implements `CrossingV1.Latch.init`.
pub(super) fn init() -> State {
    State::Idle
}

/// Implements `CrossingV1.Latch.step`.
pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {
    match (state, input) {
        (State::Idle, Input::ButtonPressed) => (State::Pending, Some(Output::Requested)),
        (State::Pending, Input::ButtonPressed) => (State::Pending, None),
        (_, Input::Acknowledged) => (State::Idle, None),
    }
}
