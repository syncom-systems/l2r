//! The CLI commands. Each returns the text to print and whether it succeeded.

use std::path::Path;

use l2r_apply::{Approval, apply, approval, approve, assess, check, recover};

use crate::report;
use crate::session::{self, Session, SessionError};

/// The outcome of a command.
pub struct Outcome {
    /// Text for standard output.
    pub stdout: String,
    /// Text for standard error.
    pub stderr: String,
    /// Process exit code.
    pub code: i32,
}

impl Outcome {
    fn ok(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            code: 0,
        }
    }

    fn refused(stderr: String) -> Self {
        Self {
            stdout: String::new(),
            stderr,
            code: 1,
        }
    }
}

fn session_error(error: SessionError) -> Outcome {
    match error {
        SessionError::Load(message) | SessionError::Plan(message) => {
            Outcome::refused(format!("{message}\n"))
        }
        SessionError::Invalid(diagnostics) => {
            let mut text: String = diagnostics.iter().map(|d| format!("{d}\n")).collect();
            text.push_str(&format!(
                "model rejected: {} finding(s)\n",
                diagnostics.len()
            ));
            Outcome::refused(text)
        }
    }
}

fn with_session(model: &Path, run: impl FnOnce(Session) -> Outcome) -> Outcome {
    match Session::open(model) {
        Ok(session) => run(session),
        Err(error) => session_error(error),
    }
}

/// `l2r validate <model>`
pub fn validate(model: &Path) -> Outcome {
    match session::validated(model) {
        Ok(v) => Outcome::ok(format!(
            "model `{}` is valid: {} components, {} message edges{}\n",
            v.model().system.id,
            v.model().components.len(),
            v.model().messages.len(),
            match v.model().ports.len() {
                0 => String::new(),
                n => format!(", {n} ports"),
            }
        )),
        Err(error) => session_error(error),
    }
}

/// `l2r plan <model> [--json]`
pub fn plan(model: &Path, json: bool) -> Outcome {
    with_session(model, |s| {
        if json {
            let mut text =
                serde_json::to_string_pretty(&s.plan.summary()).expect("summary serializes");
            text.push('\n');
            Outcome::ok(text)
        } else {
            Outcome::ok(report::plan_summary(s.model(), &s.plan))
        }
    })
}

/// `l2r preview <model> [--summary]`
pub fn preview(model: &Path, show_content: bool) -> Outcome {
    with_session(model, |s| {
        let assessment = assess(&s.plan, &s.output_dir());
        Outcome::ok(report::preview(
            s.model(),
            &s.plan,
            &assessment,
            show_content,
        ))
    })
}

/// `l2r approve <model> --digest <d> --by <name>`
pub fn approve_plan(model: &Path, digest: &str, by: &str) -> Outcome {
    with_session(model, |s| {
        let assessment = assess(&s.plan, &s.output_dir());
        match approve(&s.plan, digest, by, &assessment) {
            Ok(record) => {
                let path = s.approval_path();
                match approval::write_record(&path, &record) {
                    Ok(()) => Outcome::ok(format!(
                        "recorded approval of {} by \"{}\" in {}\n\
                     authorizes replacing or deleting {} existing generated file(s)\n\
                     (a record binds generation to this digest; it is not proof of authorization)\n",
                        record.plan_digest,
                        record.recorded_by,
                        path.display(),
                        record.authorized_changes.len()
                    )),
                    Err(e) => Outcome::refused(format!("{}: {e}\n", path.display())),
                }
            }
            Err(e) => Outcome::refused(format!("approval refused: {e}\n")),
        }
    })
}

fn recorded_approval(s: &Session) -> Result<Approval, Outcome> {
    match s.read_approval() {
        Ok(Some(record)) => Ok(record),
        Ok(None) => Err(Outcome::refused(format!(
            "no approval recorded at {}; run `l2r preview`, then `l2r approve` with the digest you reviewed\n",
            s.approval_path().display()
        ))),
        Err(e) => Err(Outcome::refused(format!("{e}\n"))),
    }
}

/// `l2r generate <model>`
pub fn generate(model: &Path) -> Outcome {
    with_session(model, |s| {
        let record = match recorded_approval(&s) {
            Ok(record) => record,
            Err(outcome) => return outcome,
        };
        match apply(&s.plan, &record, &s.output_dir()) {
            Ok(report) => {
                let mut text =
                    format!("wrote plan {} to {}/\n", s.plan.digest(), s.plan.output_dir);
                for action in &report.assessment.actions {
                    text.push_str(&format!("  {:?} {}\n", action.kind, action.path));
                }
                Outcome::ok(text)
            }
            Err(e) => Outcome::refused(format!("generation refused: {e}\n")),
        }
    })
}

/// `l2r recover <model>`
pub fn recover_generation(model: &Path) -> Outcome {
    with_session(model, |s| {
        let record = match recorded_approval(&s) {
            Ok(record) => record,
            Err(outcome) => return outcome,
        };
        match recover(&s.plan, &record, &s.output_dir()) {
            Ok(()) => Outcome::ok(format!(
                "completed interrupted generation of plan {} in {}/\n",
                s.plan.digest(),
                s.plan.output_dir
            )),
            Err(e) => Outcome::refused(format!("recovery refused: {e}\n")),
        }
    })
}

/// `l2r check <model>`
pub fn check_output(model: &Path) -> Outcome {
    with_session(model, |s| {
        let mut findings = check(s.model(), &s.plan, &s.output_dir());
        match s.read_approval() {
            Ok(Some(record)) => {
                if let Err(e) = l2r_apply::verify(&record, &s.plan) {
                    findings.push(l2r_apply::Finding {
                        path: s.approval_path().display().to_string(),
                        message: e.to_string(),
                    });
                }
            }
            Ok(None) => findings.push(l2r_apply::Finding {
                path: s.approval_path().display().to_string(),
                message: "no approval recorded".into(),
            }),
            Err(e) => findings.push(l2r_apply::Finding {
                path: s.approval_path().display().to_string(),
                message: e,
            }),
        }
        if findings.is_empty() {
            Outcome::ok(format!(
                "{}/ matches approved plan {}\n",
                s.plan.output_dir,
                s.plan.digest()
            ))
        } else {
            let text: String = findings
                .iter()
                .map(|f| format!("{}: {}\n", f.path, f.message))
                .collect();
            Outcome::refused(format!(
                "{text}check failed: {} finding(s)\n",
                findings.len()
            ))
        }
    })
}

/// `l2r render <model>`
pub fn render(model: &Path) -> Outcome {
    match session::validated(model) {
        Ok(v) => Outcome::ok(report::mermaid(v.model())),
        Err(error) => session_error(error),
    }
}

/// `l2r lean-composition <model>`: the derived Lean composition, on stdout.
pub fn lean_composition(model: &Path) -> Outcome {
    match session::validated(model) {
        Ok(v) => match l2r_plan::lean_composition(&v) {
            Ok(text) => Outcome::ok(text),
            Err(e) => Outcome::refused(format!("{e}\n")),
        },
        Err(error) => session_error(error),
    }
}
