//! The approval boundary and safe writing of generation plans.
//!
//! Nothing reaches the disk unless a recorded [`Approval`] names the exact plan
//! digest and an [`Assessment`] of the output directory finds no conflict.
//! Generated files are replaced only while they still match what L2R wrote;
//! implementation files are created once and never touched again.

pub mod approval;
mod assess;
mod check;
mod fs_guard;
pub mod lock;
mod write;

pub use approval::{Approval, ApprovalError, approve, verify};
pub use assess::{Action, ActionKind, Assessment, Conflict, assess};
pub use check::{Finding, check};
pub use write::{ApplyError, Report, apply};
