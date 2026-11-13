//! How L2R writes a single file outside the output tree: the approval record,
//! the Lean inventory, the files of a new project and the exporter it runs.
//!
//! The output tree has its own descriptor-relative handle (see the crate
//! documentation). Outside it, every write goes through this module, and no
//! write follows a symbolic link or reuses a file someone else created:
//!
//! - [`replace`] stages the bytes in a file it creates exclusively beside the
//!   target, then renames it over the target. A link at the target is replaced,
//!   never followed. A file or link already at a staging name is never opened;
//!   another name is tried.
//! - [`create`] creates the target exclusively and fails if anything, a link
//!   included, is already there.
//! - [`PrivateDir`] is a fresh directory only the current user can enter, for
//!   files that must exist while a tool runs and are removed afterwards.
//!
//! This module decides how a file is written, never whether it may be. Each
//! caller keeps its own authorization policy: the approval record is written
//! by `l2r approve`, the inventory by `l2r inventory` without any approval.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// How many fresh names are tried before giving up because each was taken.
const ATTEMPTS: u32 = 16;

/// Replaces the file at `path` with `bytes`, atomically.
///
/// Readers see the old file or the new one, never a partial one. A symbolic
/// link at `path` is replaced by the new file; the file it pointed to is not
/// touched.
pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (dir, name) = split(path)?;
    let (staged, mut file) = exclusive(|| dir.join(format!(".{name}.{}.tmp", unique())))?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&staged, path));
    if written.is_err() {
        let _ = fs::remove_file(&staged);
    }
    written
}

/// Creates the file `path` with `bytes`. Fails with
/// [`io::ErrorKind::AlreadyExists`] if anything is at `path`, a dangling
/// symbolic link included.
pub fn create(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)
}

/// A directory created fresh for this process, readable and writable by the
/// current user only, and removed with its contents when dropped.
#[derive(Debug)]
pub struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    /// Creates a new directory in `parent` with mode `0700`. A name already
    /// taken, by a directory, a file or a link, is never used.
    pub fn create_in(parent: &Path) -> io::Result<PrivateDir> {
        PrivateDir::create_named(|| parent.join(format!("l2r-{}", unique())))
    }

    /// Creates the directory at the first path `name` returns that nothing
    /// occupies.
    fn create_named(name: impl Fn() -> PathBuf) -> io::Result<PrivateDir> {
        let path = attempt(|| {
            let path = name();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .map(|()| path)
        })?;
        Ok(PrivateDir { path })
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Creates the file `name` in the directory with `bytes`, and returns its
    /// path. `name` must be a single, plain file name.
    pub fn create_file(&self, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        // A plain name is its own last component: not empty, `.`, `..` or a path.
        if Path::new(name).file_name() != Some(std::ffi::OsStr::new(name)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("`{name}` is not a plain file name"),
            ));
        }
        let path = self.path.join(name);
        create(&path, bytes).map(|()| path)
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// The directory and file name of `path`.
fn split(path: &Path) -> io::Result<(&Path, String)> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} does not name a file", path.display()),
        )
    })?;
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok((dir, name.to_string_lossy().into_owned()))
}

/// Creates a file exclusively at a path `name` returns, trying fresh names
/// while each is taken.
fn exclusive(name: impl Fn() -> PathBuf) -> io::Result<(PathBuf, fs::File)> {
    attempt(|| {
        let path = name();
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map(|file| (path, file))
    })
}

/// Runs `make` until it succeeds or fails for a reason other than a name that
/// is already taken, at most [`ATTEMPTS`] times.
fn attempt<T>(make: impl Fn() -> io::Result<T>) -> io::Result<T> {
    let mut last = None;
    for _ in 0..ATTEMPTS {
        match make() {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = Some(e),
            done => return done,
        }
    }
    Err(last.expect("ATTEMPTS is positive"))
}

/// A name part no other call in this process returns, and which another
/// process is unlikely to guess. Exclusivity, not secrecy, is what makes the
/// writes safe: a guessed name is only skipped.
fn unique() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!(
        "{}.{nanos:x}.{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::fs::PermissionsExt;

    fn taken() -> io::Error {
        io::Error::from(io::ErrorKind::AlreadyExists)
    }

    #[test]
    fn a_taken_name_is_skipped_for_a_fresh_one() {
        let tries = Cell::new(0);
        let made = attempt(|| {
            tries.set(tries.get() + 1);
            if tries.get() < 3 {
                Err(taken())
            } else {
                Ok(tries.get())
            }
        });
        assert_eq!(made.unwrap(), 3);
    }

    #[test]
    fn other_errors_are_not_retried() {
        let tries = Cell::new(0);
        let made: io::Result<()> = attempt(|| {
            tries.set(tries.get() + 1);
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        });
        assert_eq!(made.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(tries.get(), 1);
    }

    #[test]
    fn attempts_are_bounded() {
        let tries = Cell::new(0);
        let made: io::Result<()> = attempt(|| {
            tries.set(tries.get() + 1);
            Err(taken())
        });
        assert_eq!(made.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(tries.get(), ATTEMPTS);
    }

    #[test]
    fn a_link_at_a_staging_name_is_skipped_and_never_followed() {
        let dir = std::env::temp_dir().join(format!("l2r-file-unit-{}", unique()));
        fs::create_dir(&dir).unwrap();
        let victim = dir.join("victim");
        fs::write(&victim, "victim").unwrap();
        let planted = dir.join("planted");
        std::os::unix::fs::symlink(&victim, &planted).unwrap();
        let tries = Cell::new(0);
        let (staged, mut file) = exclusive(|| {
            tries.set(tries.get() + 1);
            if tries.get() == 1 {
                planted.clone()
            } else {
                dir.join("fresh")
            }
        })
        .unwrap();
        file.write_all(b"staged").unwrap();
        assert_eq!(staged, dir.join("fresh"));
        assert_eq!(tries.get(), 2);
        assert_eq!(fs::read_to_string(&victim).unwrap(), "victim");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_link_or_directory_at_a_private_dir_name_is_skipped_and_never_followed() {
        // What an attacker sharing the temporary directory could plant at the
        // name the exporter's directory would take: a link to a directory they
        // control, or a directory of their own.
        let base = std::env::temp_dir().join(format!("l2r-file-unit-{}", unique()));
        fs::create_dir(&base).unwrap();
        let theirs = base.join("theirs");
        fs::create_dir(&theirs).unwrap();
        let linked = base.join("linked");
        std::os::unix::fs::symlink(&theirs, &linked).unwrap();
        let tries = Cell::new(0);
        let candidates = [linked.clone(), theirs.clone(), base.join("fresh")];
        let dir = PrivateDir::create_named(|| {
            tries.set(tries.get() + 1);
            candidates[tries.get() - 1].clone()
        })
        .unwrap();
        assert_eq!(dir.path(), base.join("fresh"));
        dir.create_file("Exporter.lean", b"x").unwrap();
        assert_eq!(
            fs::read_dir(&theirs).unwrap().count(),
            0,
            "nothing written into theirs"
        );
        let mode = fs::metadata(dir.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        drop(dir);
        assert!(
            theirs.exists() && linked.exists(),
            "planted entries are left alone"
        );
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn names_are_unique_within_a_process() {
        assert_ne!(unique(), unique());
    }
}
