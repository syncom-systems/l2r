//! Drift and boundary checks of an output directory against the current plan.

use std::path::Path;

use l2r_model::Model;
use l2r_plan::{Ownership, Plan};

use crate::assess::{ActionKind, assess};
use crate::fs_guard::{self, Occupant};
use crate::lock::LOCK_FILE;

/// One problem found by [`check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Path relative to the output directory.
    pub path: String,
    /// Explanation.
    pub message: String,
}

/// Compares `root` with `plan`: generated files must match exactly, the lock
/// must describe this plan, implementation files must exist and stay within
/// their component's size budget.
pub fn check(model: &Model, plan: &Plan, root: &Path) -> Vec<Finding> {
    let assessment = assess(plan, root);
    let mut findings: Vec<Finding> = assessment
        .conflicts
        .iter()
        .map(|c| Finding {
            path: c.path.clone(),
            message: c.reason.clone(),
        })
        .collect();
    for action in &assessment.actions {
        let message = match action.kind {
            ActionKind::Create => "planned file is missing",
            ActionKind::Update => "generated file is out of date with the current plan",
            ActionKind::Delete => "generated file is no longer planned",
            ActionKind::Orphan => "implementation file is no longer planned by the model",
            ActionKind::Unchanged | ActionKind::Keep => continue,
        };
        findings.push(Finding {
            path: action.path.clone(),
            message: message.into(),
        });
    }
    findings.extend(check_lock(plan, root));
    findings.extend(check_budgets(model, plan, root));
    findings.sort_by(|a, b| a.path.cmp(&b.path).then(a.message.cmp(&b.message)));
    findings
}

fn check_lock(plan: &Plan, root: &Path) -> Option<Finding> {
    let finding = |message: String| Finding {
        path: LOCK_FILE.into(),
        message,
    };
    match crate::assess::read_lock(root) {
        Ok(Some(lock)) if lock.plan_digest == plan.digest() => None,
        Ok(Some(lock)) => Some(finding(format!(
            "lock records plan {}, not the current plan {}",
            lock.plan_digest,
            plan.digest()
        ))),
        Ok(None) => Some(finding(
            "no generation lock; the plan has not been applied here".into(),
        )),
        Err(_) => None, // already reported as a conflict
    }
}

fn check_budgets(model: &Model, plan: &Plan, root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    for artifact in plan
        .artifacts
        .iter()
        .filter(|a| a.ownership == Ownership::Implementation)
    {
        let budget = artifact
            .components
            .first()
            .and_then(|id| model.component(id))
            .and_then(|c| c.implementation_budget_lines);
        let Some(budget) = budget else { continue };
        if let Ok(Occupant::File(bytes)) = fs_guard::inspect(root, &artifact.path) {
            let lines = String::from_utf8_lossy(&bytes).lines().count();
            if lines > budget as usize {
                findings.push(Finding {
                    path: artifact.path.clone(),
                    message: format!("{lines} lines exceed the architect's budget of {budget}; split the work or ask for a revised architecture"),
                });
            }
        }
    }
    findings
}
