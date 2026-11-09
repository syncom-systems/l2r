//! Implementation of Crossing signal (`signal`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or
//! public items, and report contradictions with the Lean specification instead
//! of working around them.
//!
//! Size budget: at most 80 lines (checked by `l2r check`).

use super::{Input, Output, State};

/// Implements `CrossingV1.Signal.init`.
pub(super) fn init() -> State {
    State::VehiclesGreen
}

/// Implements `CrossingV1.Signal.step`.
pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {
    match (state, input) {
        (State::VehiclesGreen, Input::CrossingRequested) => (State::VehiclesAmber, None),
        (State::VehiclesAmber, Input::CrossingRequested) => (State::VehiclesAmber, None),
        (State::Walk, Input::CrossingRequested) => (State::WalkQueued, None),
        (State::WalkQueued, Input::CrossingRequested) => (State::WalkQueued, None),
        (State::Clearance, Input::CrossingRequested) => (State::ClearanceQueued, None),
        (State::ClearanceQueued, Input::CrossingRequested) => (State::ClearanceQueued, None),
        (State::VehiclesGreen, Input::TimerExpired) => (State::VehiclesGreen, None),
        (State::VehiclesAmber, Input::TimerExpired) => (State::Walk, Some(Output::WalkStarted)),
        (State::Walk, Input::TimerExpired) => (State::Clearance, None),
        (State::WalkQueued, Input::TimerExpired) => (State::ClearanceQueued, None),
        (State::Clearance, Input::TimerExpired) => (State::VehiclesGreen, None),
        (State::ClearanceQueued, Input::TimerExpired) => (State::VehiclesAmber, None),
    }
}
