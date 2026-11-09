//! Implementation of High-level alarm (`alarm`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or
//! public items, and report contradictions with the Lean specification instead
//! of working around them.
//!
//! Size budget: at most 60 lines (checked by `l2r check`).

use super::{Input, Output, State};

/// Implements `Tank.Alarm.init`.
pub(super) fn init() -> State {
    State {
        active: false,
        count: 0,
    }
}

/// Implements `Tank.Alarm.bump`: `n + 1`, staying at the largest value.
fn bump(n: u8) -> u8 {
    if n == 255 { 255 } else { n + 1 }
}

/// Implements `Tank.Alarm.step`.
pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {
    match input {
        Input::High => {
            if state.active {
                (state, None)
            } else {
                let count = bump(state.count);
                let next = State {
                    active: true,
                    count,
                };
                (next, Some(Output::Raised(count)))
            }
        }
        Input::Cleared => (state, None),
        Input::Acknowledge => {
            if state.active {
                let next = State {
                    active: false,
                    count: state.count,
                };
                (next, Some(Output::Silenced))
            } else {
                (state, None)
            }
        }
    }
}
