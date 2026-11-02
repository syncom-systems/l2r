//! Reading the output directory without following symlinks out of it.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// What occupies a planned path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Occupant {
    /// Nothing; the path (or one of its parent directories) does not exist.
    Absent,
    /// A regular file with this content.
    File(Vec<u8>),
}

/// Inspects `relative` under `root`, refusing symlinks and non-directories on
/// the way. `relative` must already be a validated safe relative path.
pub fn inspect(root: &Path, relative: &str) -> Result<Occupant, String> {
    check_root(root)?;
    let segments: Vec<&str> = relative.split('/').collect();
    let mut current = root.to_path_buf();
    for (index, segment) in segments.iter().enumerate() {
        current.push(segment);
        let is_last = index + 1 == segments.len();
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Occupant::Absent),
            Err(e) => return Err(format!("cannot inspect `{}`: {e}", display(&current, root))),
        };
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "`{}` is a symbolic link; L2R does not write through links",
                display(&current, root)
            ));
        }
        if !is_last && !metadata.is_dir() {
            return Err(format!("`{}` is not a directory", display(&current, root)));
        }
        if is_last && !metadata.is_file() {
            return Err(format!(
                "`{}` exists and is not a regular file",
                display(&current, root)
            ));
        }
    }
    std::fs::read(&current)
        .map(Occupant::File)
        .map_err(|e| format!("cannot read `{relative}`: {e}"))
}

/// The output directory must be absent or a real directory, never a link.
pub fn check_root(root: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(root) {
        Ok(m) if m.file_type().is_symlink() => Err(format!(
            "output directory `{}` is a symbolic link",
            root.display()
        )),
        Ok(m) if !m.is_dir() => Err(format!(
            "output directory `{}` is not a directory",
            root.display()
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!(
            "cannot inspect output directory `{}`: {e}",
            root.display()
        )),
    }
}

fn display(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(PathBuf::from)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}
