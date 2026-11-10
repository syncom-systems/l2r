//! Implementation of Valve controller (`controller`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or
//! public items, and report contradictions with the Lean specification instead
//! of working around them.
//!
//! Size budget: at most 80 lines (checked by `l2r check`).

use super::{Input, Output, State};
use crate::types::tank::Level;
use crate::types::tank::controller::{Command, Mode, Valve};

/// Implements `Tank.Controller.init`.
pub(super) fn init() -> State {
    State {
        mode: Mode::Auto,
        valve: Valve::Closed,
        level: Level::Normal,
    }
}

/// Implements `Tank.Controller.position`.
fn position(mode: Mode, valve: Valve, level: Level) -> Valve {
    match level {
        Level::High => Valve::Closed,
        Level::Low => match mode {
            Mode::Auto => Valve::Opened,
            Mode::Manual => valve,
        },
        Level::Normal => valve,
    }
}

/// `v != w` on valve positions, without a trait call.
fn differs(v: Valve, w: Valve) -> bool {
    match (v, w) {
        (Valve::Closed, Valve::Closed) => false,
        (Valve::Opened, Valve::Opened) => false,
        (Valve::Closed, Valve::Opened) => true,
        (Valve::Opened, Valve::Closed) => true,
    }
}

/// Implements `Tank.Controller.settle`.
fn settle(state: State, mode: Mode, wanted: Valve, level: Level) -> (State, Option<Output>) {
    let valve = position(mode, wanted, level);
    let next = State { mode, valve, level };
    let report = Output {
        valve,
        level,
        changed: differs(valve, state.valve),
    };
    (next, Some(report))
}

/// Implements `Tank.Controller.step`.
pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {
    match input {
        Input::Level(level) => settle(state, state.mode, state.valve, level),
        Input::Command(Command::SetAuto) => settle(state, Mode::Auto, state.valve, state.level),
        Input::Command(Command::ForceOpen) => {
            settle(state, Mode::Manual, Valve::Opened, state.level)
        }
        Input::Command(Command::ForceClose) => {
            settle(state, Mode::Manual, Valve::Closed, state.level)
        }
    }
}
