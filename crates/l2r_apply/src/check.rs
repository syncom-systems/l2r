//! Drift and boundary checks of an output directory against the current plan.

use std::path::Path;

use l2r_model::Model;
use l2r_plan::{Ownership, Plan};

use crate::assess::{ActionKind, Assessment, assess_tree, read_lock};
use crate::lock::LOCK_FILE;
use crate::tree::{Occupant, Tree};

/// One problem found by [`check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Path relative to the output directory (or a state file name).
    pub path: String,
    /// Explanation.
    pub message: String,
}

fn finding(path: impl Into<String>, message: impl Into<String>) -> Finding {
    Finding {
        path: path.into(),
        message: message.into(),
    }
}

/// Compares `root` with `plan`: generated files must match exactly, the lock
/// must describe this plan, no generation may be pending, and implementation
/// files must exist and stay within their component's size budget. Every read
/// goes through [`Tree`], so nothing outside the tree is inspected.
pub fn check(model: &Model, plan: &Plan, root: &Path) -> Vec<Finding> {
    let tree = match Tree::open(root) {
        Ok(Some(tree)) => tree,
        Ok(None) => {
            return vec![finding(
                ".",
                "output directory does not exist; the plan has not been applied",
            )];
        }
        Err(e) => return vec![finding(e.path, e.reason)],
    };
    let assessment = assess_tree(plan, &tree);
    let mut findings = assessment_findings(&assessment);
    findings.extend(check_lock(plan, &tree));
    findings.extend(check_budgets(model, plan, &tree));
    findings.sort_by(|a, b| a.path.cmp(&b.path).then(a.message.cmp(&b.message)));
    findings.dedup();
    findings
}

fn assessment_findings(assessment: &Assessment) -> Vec<Finding> {
    let mut findings: Vec<Finding> = assessment
        .conflicts
        .iter()
        .map(|c| finding(&c.path, &c.reason))
        .collect();
    for action in &assessment.actions {
        let message = match action.kind {
            ActionKind::Create => "planned file is missing",
            ActionKind::Update => "generated file is out of date with the current plan",
            ActionKind::Delete => "generated file is no longer planned",
            ActionKind::Orphan => "implementation file is no longer planned by the model",
            ActionKind::Unchanged | ActionKind::Keep => continue,
        };
        findings.push(finding(action.path.as_str(), message));
    }
    findings
}

fn check_lock(plan: &Plan, tree: &Tree) -> Option<Finding> {
    match read_lock(tree) {
        Ok(Some(lock)) if lock.plan_digest == plan.digest() => None,
        Ok(Some(lock)) => Some(finding(
            LOCK_FILE,
            format!(
                "lock records plan {}, not the current plan {}",
                lock.plan_digest,
                plan.digest()
            ),
        )),
        Ok(None) => Some(finding(
            LOCK_FILE,
            "no generation lock; the plan has not been applied here",
        )),
        Err(_) => None, // already reported as a conflict
    }
}

fn check_budgets(model: &Model, plan: &Plan, tree: &Tree) -> Vec<Finding> {
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
        if let Ok(Occupant::File(bytes)) = tree.read(&artifact.path) {
            let lines = String::from_utf8_lossy(&bytes).lines().count();
            if lines > budget as usize {
                findings.push(finding(
                    artifact.path.as_str(),
                    format!(
                        "{lines} lines exceed the architect's budget of {budget}; split the work or ask for a \
                         revised architecture"
                    ),
                ));
            }
        }
    }
    findings
}
