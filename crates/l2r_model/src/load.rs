//! Reading models and inventories from disk.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::architecture::Model;
use crate::lean::Inventory;

/// A model or inventory could not be read or parsed.
#[derive(Debug)]
pub struct LoadError {
    /// File that failed to load.
    pub path: PathBuf,
    /// What went wrong.
    pub message: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for LoadError {}

fn read(path: &Path) -> Result<String, LoadError> {
    std::fs::read_to_string(path).map_err(|e| LoadError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// Parses a model from TOML text. Unknown fields are rejected.
pub fn parse_model(text: &str) -> Result<Model, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

/// Parses an inventory from JSON text. Unknown fields are rejected.
pub fn parse_inventory(text: &str) -> Result<Inventory, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// Reads and parses a model file.
pub fn load_model(path: &Path) -> Result<Model, LoadError> {
    parse_model(&read(path)?).map_err(|message| LoadError {
        path: path.to_path_buf(),
        message,
    })
}

/// Reads and parses an inventory file.
pub fn load_inventory(path: &Path) -> Result<Inventory, LoadError> {
    parse_inventory(&read(path)?).map_err(|message| LoadError {
        path: path.to_path_buf(),
        message,
    })
}
