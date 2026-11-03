//! Fault injection for testing interrupted generations.
//!
//! Compiled in only with the `fault-injection` crate feature, which the shipped
//! `l2r` binary never enables (see `crates/l2r_apply/BUILD.bazel`). Without the
//! feature, [`checkpoint`] is an empty function.

use crate::tree::TreeError;

#[cfg(feature = "fault-injection")]
mod armed {
    use std::cell::Cell;

    /// How an armed fault stops the run.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Mode {
        /// Return an error, as if the process died: nothing further is written.
        Interrupt,
        /// Abort the process.
        Abort,
        /// Fail while staging the given (1-based) file, before any commit.
        FailStaging,
    }

    thread_local! {
        static ARMED: Cell<Option<(usize, Mode)>> = const { Cell::new(None) };
        static RUN_ID: Cell<Option<&'static str>> = const { Cell::new(None) };
    }

    /// Makes this thread's temporary names predictable (`.l2r-tmp.<id>.<n>`), so a
    /// test can plant files or links at exactly the names L2R will create.
    pub fn fix_run_id(id: &'static str) {
        RUN_ID.with(|r| r.set(Some(id)));
    }

    pub(crate) fn run_id() -> Option<&'static str> {
        RUN_ID.with(|r| r.get())
    }

    /// Stops the current thread's next generation after `steps` committed steps.
    pub fn arm(steps: usize, mode: Mode) {
        ARMED.with(|a| a.set(Some((steps, mode))));
    }

    /// Disarms any pending fault.
    pub fn disarm() {
        ARMED.with(|a| a.set(None));
    }

    pub(super) fn fire(completed: usize, staging: bool) -> Option<Mode> {
        ARMED.with(|a| match a.get() {
            Some((at, mode)) if at == completed && (mode == Mode::FailStaging) == staging => {
                a.set(None);
                Some(mode)
            }
            _ => None,
        })
    }
}

#[cfg(feature = "fault-injection")]
pub use armed::{Mode, arm, disarm, fix_run_id};

/// Called after each committed step (and after the lock is written).
#[cfg(feature = "fault-injection")]
pub fn checkpoint(completed: usize) -> Result<(), TreeError> {
    match armed::fire(completed, false) {
        Some(Mode::Abort) => std::process::abort(),
        Some(_) => Err(TreeError {
            path: "<fault injection>".into(),
            reason: format!("interrupted after {completed} step(s)"),
        }),
        None => Ok(()),
    }
}

/// Called before staging each file (1-based).
#[cfg(feature = "fault-injection")]
pub fn staging_checkpoint(index: usize) -> Result<(), TreeError> {
    match armed::fire(index, true) {
        Some(_) => Err(TreeError {
            path: "<fault injection>".into(),
            reason: format!("staging failed at file {index}"),
        }),
        None => Ok(()),
    }
}

/// Called before staging each file (1-based).
#[cfg(not(feature = "fault-injection"))]
#[inline(always)]
pub fn staging_checkpoint(_index: usize) -> Result<(), TreeError> {
    Ok(())
}

/// Called after each committed step (and after the lock is written).
#[cfg(not(feature = "fault-injection"))]
#[inline(always)]
pub fn checkpoint(_completed: usize) -> Result<(), TreeError> {
    Ok(())
}

/// A fixed run id for temporary names, set only by tests in the fault build.
#[cfg(feature = "fault-injection")]
pub(crate) fn run_id_override() -> Option<String> {
    armed::run_id().map(str::to_string)
}

/// A fixed run id for temporary names, set only by tests in the fault build.
#[cfg(not(feature = "fault-injection"))]
#[inline(always)]
pub(crate) fn run_id_override() -> Option<String> {
    None
}
