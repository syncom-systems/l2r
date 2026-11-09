//! Loading a model, its inventory and its plan from the paths the model names.

use std::path::{Path, PathBuf};

use l2r_apply::{Approval, approval};
use l2r_model::{Model, load};
use l2r_plan::{GeneratorIdentity, Plan, plan};
use l2r_validate::{Diagnostic, Validated, validate};

/// Why a session could not be opened.
#[derive(Debug)]
pub enum SessionError {
    /// A file could not be read or parsed.
    Load(String),
    /// The model is invalid.
    Invalid(Vec<Diagnostic>),
    /// Planning failed.
    Plan(String),
}

/// A validated model together with its plan and file locations.
pub struct Session {
    /// The model file.
    pub model_path: PathBuf,
    /// The validated, canonical model.
    pub validated: Validated,
    /// The deterministic plan.
    pub plan: Plan,
}

impl Session {
    /// Loads and validates the model at `model_path`, then plans it.
    pub fn open(model_path: &Path) -> Result<Self, SessionError> {
        let validated = validated(model_path)?;
        let plan = plan(&validated, &GeneratorIdentity::current())
            .map_err(|e| SessionError::Plan(e.to_string()))?;
        Ok(Self {
            model_path: model_path.to_path_buf(),
            validated,
            plan,
        })
    }

    /// The model.
    pub fn model(&self) -> &Model {
        self.validated.model()
    }

    /// The output directory named by the model.
    pub fn output_dir(&self) -> PathBuf {
        base_dir(&self.model_path).join(&self.model().system.output_dir)
    }

    /// Where the approval record for this model lives.
    pub fn approval_path(&self) -> PathBuf {
        approval_path(&self.model_path)
    }

    /// Reads the recorded approval, if one exists.
    pub fn read_approval(&self) -> Result<Option<Approval>, String> {
        let path = self.approval_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => approval::from_json(&text)
                .map(Some)
                .map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

/// Loads and validates without planning.
pub fn validated(model_path: &Path) -> Result<Validated, SessionError> {
    let model = load::load_model(model_path).map_err(|e| SessionError::Load(e.to_string()))?;
    // Path fields are validated before they are used to open anything.
    if let Err(diagnostics) = validate_paths(&model) {
        return Err(SessionError::Invalid(diagnostics));
    }
    let inventory_path = base_dir(model_path).join(&model.system.lean_inventory);
    let inventory =
        load::load_inventory(&inventory_path).map_err(|e| SessionError::Load(e.to_string()))?;
    validate(&model, &inventory).map_err(SessionError::Invalid)
}

fn validate_paths(model: &Model) -> Result<(), Vec<Diagnostic>> {
    let system = &model.system;
    let unsafe_paths: Vec<Diagnostic> = [
        ("system.lean_inventory", &system.lean_inventory),
        ("system.output_dir", &system.output_dir),
    ]
    .into_iter()
    .filter(|(_, path)| !l2r_validate::names::is_safe_relative_path(path))
    .map(|(field, path)| Diagnostic {
        code: "L2R-S013",
        subject: field.into(),
        message: format!("`{path}` must be a relative path without `..` or hidden segments"),
    })
    .collect();
    if unsafe_paths.is_empty() {
        Ok(())
    } else {
        Err(unsafe_paths)
    }
}

fn base_dir(model_path: &Path) -> PathBuf {
    model_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// `dir/architecture.toml` → `dir/architecture.approval.json`.
pub fn approval_path(model_path: &Path) -> PathBuf {
    let stem = model_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".into());
    base_dir(model_path).join(format!("{stem}.approval.json"))
}
