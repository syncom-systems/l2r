//! Implementation of Monitor (`monitor`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or
//! public items, and report contradictions with the Lean specification instead
//! of working around them.
//!
//! Size budget: at most 60 lines (checked by `l2r check`).

use super::{Input, Output, State};

/// Implements `Starter.Monitor.limit`.
const LIMIT: u8 = 100;

/// Implements `Starter.Monitor.init`.
pub(super) fn init() -> State {
    State { last: 0, alerts: 0 }
}

/// Implements `Starter.Monitor.bump`: `n + 1`, staying at the largest value.
fn bump(n: u8) -> u8 {
    if n == 255 { 255 } else { n + 1 }
}

/// Implements `Starter.Monitor.step`.
pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {
    match input {
        Input::Reading(value) => {
            if value > LIMIT {
                let next = State {
                    last: value,
                    alerts: bump(state.alerts),
                };
                (next, Some(Output::Alert(value)))
            } else {
                let next = State {
                    last: value,
                    alerts: state.alerts,
                };
                (next, None)
            }
        }
        Input::Reset => {
            let next = State {
                last: state.last,
                alerts: 0,
            };
            (next, Some(Output::Cleared))
        }
    }
}
