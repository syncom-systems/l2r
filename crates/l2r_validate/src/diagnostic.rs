//! Validation findings.

use std::fmt;

/// One reason a model was rejected.
///
/// Codes are stable and documented in `docs/model-reference.md`; tests and
/// tooling match on them, never on message text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    /// Stable rule code, e.g. `L2R-S003`.
    pub code: &'static str,
    /// The model element the finding is about.
    pub subject: String,
    /// Explanation for the architect.
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}] {}: {}", self.code, self.subject, self.message)
    }
}

/// Accumulates diagnostics so that every problem is reported in one run.
#[derive(Debug, Default)]
pub struct Findings(Vec<Diagnostic>);

impl Findings {
    /// Records a finding.
    pub fn error(
        &mut self,
        code: &'static str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.0.push(Diagnostic {
            code,
            subject: subject.into(),
            message: message.into(),
        });
    }

    /// True when nothing has been recorded.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the findings sorted and de-duplicated.
    pub fn into_sorted(mut self) -> Vec<Diagnostic> {
        self.0.sort();
        self.0.dedup();
        self.0
    }
}
