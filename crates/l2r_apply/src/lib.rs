//! The approval boundary and safe writing of generation plans.
//!
//! Nothing reaches the disk unless a recorded [`Approval`] names the exact plan
//! digest and an [`Assessment`] of the output tree finds no conflict. All access
//! to the tree goes through one descriptor-relative, no-follow handle, and every
//! path, whether planned, read from the lock or read from the journal, is a
//! validated [`l2r_model::RelPath`]. Generated files are replaced only while they
//! still match what L2R wrote; implementation files are created once and never
//! touched again. See [`transaction`] for the exact interruption guarantees.
//! Files outside the output tree are written through [`file`].

#[cfg(not(unix))]
compile_error!(
    "the L2R write boundary uses openat-family system calls and requires a Unix platform"
);

pub mod approval;
mod assess;
mod check;
pub mod fault;
pub mod file;
pub mod journal;
pub mod lock;
pub mod transaction;
mod tree;

pub use approval::{
    Approval, ApprovalError, AuthorizedChange, Change, approve, destructive_changes, verify,
};
pub use assess::{Action, ActionKind, Assessment, Conflict, assess};
pub use check::{Finding, check};
pub use transaction::{ApplyError, Report, apply, pending, recover};
pub use tree::{Occupant, TempName, TreeError};
