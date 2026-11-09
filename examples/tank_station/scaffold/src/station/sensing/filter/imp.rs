//! Implementation of Level filter (`filter`).
//!
//! This file belongs to the implementer. L2R created it once and will never
//! overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or
//! public items, and report contradictions with the Lean specification instead
//! of working around them.
//!
//! Size budget: at most 80 lines (checked by `l2r check`).

use super::{Input, Output, State};
use crate::types::tank::Level;

/// Implements `Tank.Filter.confirmAfter`.
const CONFIRM_AFTER: u8 = 2;

/// Implements `Tank.Filter.init`.
pub(super) fn init() -> State {
    State {
        reported: Level::Normal,
        candidate: Level::Normal,
        streak: 0,
    }
}

/// Implements `Tank.Filter.classify`.
fn classify(raw: u8) -> Level {
    if raw < 20 {
        Level::Low
    } else if raw > 200 {
        Level::High
    } else {
        Level::Normal
    }
}

/// Implements `Tank.Filter.bump`: `n + 1`, staying at the largest value.
fn bump(n: u8) -> u8 {
    if n == 255 { 255 } else { n + 1 }
}

/// Equality of levels, without a trait call.
fn same(a: Level, b: Level) -> bool {
    matches!(
        (a, b),
        (Level::Low, Level::Low) | (Level::Normal, Level::Normal) | (Level::High, Level::High)
    )
}

/// The state after a sample classified as `candidate`.
fn moved(reported: Level, candidate: Level, streak: u8) -> State {
    State {
        reported,
        candidate,
        streak,
    }
}

/// Implements `Tank.Filter.step`.
pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {
    let Input::Sample(raw) = input;
    let level = classify(raw);
    if same(level, state.reported) {
        (moved(state.reported, level, 0), None)
    } else if same(level, state.candidate) {
        let streak = bump(state.streak);
        if streak >= CONFIRM_AFTER {
            (moved(level, level, 0), Some(Output::LevelChanged(level)))
        } else {
            (moved(state.reported, level, streak), None)
        }
    } else {
        (moved(state.reported, level, 1), None)
    }
}
