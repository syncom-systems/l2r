//! Writing an approved plan.

use std::fmt;
use std::path::Path;

use l2r_plan::Plan;

use crate::approval::{self, Approval, ApprovalError};
use crate::assess::{ActionKind, Assessment, Conflict, assess};
use crate::lock::{LOCK_FILE, Lock};

/// Why a plan was not written.
#[derive(Debug)]
pub enum ApplyError {
    /// The approval does not cover this plan.
    Approval(ApprovalError),
    /// The disk is not in a state the plan can be applied to. Nothing was written.
    Conflicts(Vec<Conflict>),
    /// A write failed part-way; the message names the path.
    Io(String),
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Approval(e) => write!(f, "{e}"),
            Self::Conflicts(conflicts) => {
                writeln!(
                    f,
                    "refusing to write: {} conflict(s); nothing was written",
                    conflicts.len()
                )?;
                for c in conflicts {
                    writeln!(f, "  {}: {}", c.path, c.reason)?;
                }
                Ok(())
            }
            Self::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ApplyError {}

/// The result of a successful write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The assessment that was carried out.
    pub assessment: Assessment,
}

/// Writes `plan` under `root`, provided `approval` covers it and no conflict exists.
///
/// Every action is assessed before the first write, so a conflict anywhere
/// means nothing is written. Each file is written to a temporary sibling and
/// renamed into place. A crash between files can leave a partial update, which
/// the next assessment detects through the lock.
pub fn apply(plan: &Plan, approval: &Approval, root: &Path) -> Result<Report, ApplyError> {
    approval::verify(approval, plan).map_err(ApplyError::Approval)?;
    let assessment = assess(plan, root);
    if !assessment.conflicts.is_empty() {
        return Err(ApplyError::Conflicts(assessment.conflicts));
    }
    std::fs::create_dir_all(root).map_err(|e| io_error(root, e))?;
    for action in &assessment.actions {
        let target = root.join(&action.path);
        match action.kind {
            ActionKind::Create | ActionKind::Update => {
                let artifact = plan
                    .artifact(&action.path)
                    .expect("planned action has an artifact");
                write_file(&target, artifact.content.as_bytes())?;
            }
            ActionKind::Delete => {
                std::fs::remove_file(&target).map_err(|e| io_error(&target, e))?
            }
            ActionKind::Unchanged | ActionKind::Keep | ActionKind::Orphan => {}
        }
    }
    write_file(
        &root.join(LOCK_FILE),
        Lock::for_plan(plan).to_json().as_bytes(),
    )?;
    Ok(Report { assessment })
}

fn write_file(target: &Path, bytes: &[u8]) -> Result<(), ApplyError> {
    let parent = target.parent().expect("target has a parent");
    std::fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    let name = target
        .file_name()
        .expect("target has a name")
        .to_string_lossy();
    let temporary = parent.join(format!(".{name}.l2r-tmp"));
    std::fs::write(&temporary, bytes).map_err(|e| io_error(&temporary, e))?;
    std::fs::rename(&temporary, target).map_err(|e| io_error(target, e))
}

fn io_error(path: &Path, error: std::io::Error) -> ApplyError {
    ApplyError::Io(format!("{}: {error}", path.display()))
}
