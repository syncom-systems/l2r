//! The only code that touches an output tree.
//!
//! Every operation starts from a directory file descriptor for the tree's root
//! and walks one segment at a time with `O_DIRECTORY | O_NOFOLLOW`, then acts on
//! the final name relative to the parent's descriptor (`openat`, `linkat`,
//! `renameat`, `unlinkat`). A symbolic link anywhere below the root is never
//! followed, even if it appears after L2R has inspected the tree. Paths are
//! [`RelPath`]s or [`TempName`]s, so no unvalidated name reaches a system call.

use std::fmt;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use rustix::fs::{AtFlags, CWD, FileType, Mode, OFlags};
use rustix::io::Errno;

use l2r_model::RelPath;

/// Largest file L2R reads from an output tree.
const MAX_READ: u64 = 16 * 1024 * 1024;

const DIR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// What occupies a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Occupant {
    /// Nothing; the path or one of its parent directories does not exist.
    Absent,
    /// A regular file with this content.
    File(Vec<u8>),
}

/// An operation on the tree failed or was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeError {
    /// Path relative to the tree root.
    pub path: String,
    /// Explanation.
    pub reason: String,
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for TreeError {}

fn error(path: &str, reason: impl Into<String>) -> TreeError {
    TreeError {
        path: path.to_string(),
        reason: reason.into(),
    }
}

fn describe(errno: Errno) -> String {
    match errno {
        Errno::LOOP | Errno::NOTDIR => {
            "is a symbolic link or not a directory; L2R does not follow links".into()
        }
        Errno::EXIST => "already exists".into(),
        other => std::io::Error::from(other).to_string(),
    }
}

/// The name of a temporary file: `.l2r-tmp.<run>.<n>`, never a valid [`RelPath`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TempName(String);

impl TempName {
    /// Validates a temporary name read from a journal.
    pub fn parse(name: &str) -> Result<TempName, String> {
        let rest = name
            .strip_prefix(".l2r-tmp.")
            .ok_or("missing `.l2r-tmp.` prefix")?;
        let (run, n) = rest.split_once('.').ok_or("missing counter")?;
        let hex = !run.is_empty() && run.len() <= 32 && run.chars().all(|c| c.is_ascii_hexdigit());
        let digits = !n.is_empty() && n.len() <= 6 && n.chars().all(|c| c.is_ascii_digit());
        if hex && digits {
            Ok(TempName(name.to_string()))
        } else {
            Err(format!(
                "malformed temporary name `{}`",
                name.escape_debug()
            ))
        }
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TempName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        TempName::parse(&value)
    }
}

impl From<TempName> for String {
    fn from(name: TempName) -> String {
        name.0
    }
}

/// Allocates temporary names unique to one run of one process.
pub struct TempNames {
    run: String,
    next: AtomicU32,
}

impl TempNames {
    /// A fresh allocator. Uniqueness only avoids clashes: every temporary file is
    /// created exclusively, so a planted file of the same name causes a refusal,
    /// never a redirected write.
    pub fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let run = crate::fault::run_id_override()
            .unwrap_or_else(|| format!("{:x}{:x}", std::process::id(), nanos & 0xffff_ffff_ffff));
        Self {
            run,
            next: AtomicU32::new(0),
        }
    }

    /// The next name.
    pub fn next(&self) -> TempName {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        TempName(format!(".l2r-tmp.{}.{}", self.run, n))
    }
}

impl Default for TempNames {
    fn default() -> Self {
        Self::new()
    }
}

/// An open output tree.
pub struct Tree {
    root: OwnedFd,
}

impl Tree {
    /// Opens `root` if it exists. `root` itself must be a directory, not a link.
    pub fn open(root: &Path) -> Result<Option<Tree>, TreeError> {
        match rustix::fs::openat(CWD, root, DIR_FLAGS, Mode::empty()) {
            Ok(fd) => Ok(Some(Tree { root: fd })),
            Err(Errno::NOENT) => Ok(None),
            Err(e) => Err(error(
                &root.display().to_string(),
                format!("output directory {}", describe(e)),
            )),
        }
    }

    /// Opens `root`, creating it (one level) if it does not exist.
    pub fn open_or_create(root: &Path) -> Result<Tree, TreeError> {
        match rustix::fs::mkdirat(CWD, root, Mode::from_raw_mode(0o755)) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(e) => return Err(error(&root.display().to_string(), describe(e))),
        }
        Tree::open(root)?
            .ok_or_else(|| error(&root.display().to_string(), "vanished after creation"))
    }

    /// Opens the directory `segments` below the root, optionally creating it.
    /// Returns `None` if it does not exist and `create` is false.
    fn dir(&self, segments: &[&str], create: bool) -> Result<Option<OwnedFd>, TreeError> {
        let mut current = rustix::io::dup(&self.root).map_err(|e| error(".", describe(e)))?;
        let mut walked = String::new();
        for segment in segments {
            if !walked.is_empty() {
                walked.push('/');
            }
            walked.push_str(segment);
            if create {
                match rustix::fs::mkdirat(&current, *segment, Mode::from_raw_mode(0o755)) {
                    Ok(()) | Err(Errno::EXIST) => {}
                    Err(e) => return Err(error(&walked, describe(e))),
                }
            }
            current = match rustix::fs::openat(&current, *segment, DIR_FLAGS, Mode::empty()) {
                Ok(fd) => fd,
                Err(Errno::NOENT) if !create => return Ok(None),
                Err(e) => return Err(error(&walked, describe(e))),
            };
        }
        Ok(Some(current))
    }

    fn read_at(dir: &OwnedFd, name: &str, shown: &str) -> Result<Occupant, TreeError> {
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let fd = match rustix::fs::openat(dir, name, flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Ok(Occupant::Absent),
            Err(e) => return Err(error(shown, describe(e))),
        };
        let stat = rustix::fs::fstat(&fd).map_err(|e| error(shown, describe(e)))?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(error(shown, "exists and is not a regular file"));
        }
        let mut bytes = Vec::new();
        std::fs::File::from(fd)
            .take(MAX_READ + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| error(shown, e.to_string()))?;
        if bytes.len() as u64 > MAX_READ {
            return Err(error(shown, "larger than 16 MiB"));
        }
        Ok(Occupant::File(bytes))
    }

    /// Reads a file in the tree.
    pub fn read(&self, path: &RelPath) -> Result<Occupant, TreeError> {
        let (dirs, name) = path.split();
        match self.dir(&dirs, false)? {
            None => Ok(Occupant::Absent),
            Some(dir) => Self::read_at(&dir, name, path.as_str()),
        }
    }

    /// Reads a temporary file staged beside `path`.
    pub fn read_temp(&self, path: &RelPath, temp: &TempName) -> Result<Occupant, TreeError> {
        let (dirs, _) = path.split();
        match self.dir(&dirs, false)? {
            None => Ok(Occupant::Absent),
            Some(dir) => Self::read_at(&dir, temp.as_str(), path.as_str()),
        }
    }

    /// Reads an L2R state file at the root (`l2r.lock.json`, `l2r.journal.json`).
    pub fn read_state(&self, name: &'static str) -> Result<Occupant, TreeError> {
        Self::read_at(&self.root, name, name)
    }

    fn write_new(dir: &OwnedFd, name: &str, bytes: &[u8], shown: &str) -> Result<(), TreeError> {
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let fd = rustix::fs::openat(dir, name, flags, Mode::from_raw_mode(0o644)).map_err(|e| {
            error(
                shown,
                format!("cannot create temporary file: {}", describe(e)),
            )
        })?;
        let mut file = std::fs::File::from(fd);
        file.write_all(bytes)
            .map_err(|e| error(shown, e.to_string()))?;
        file.sync_all().map_err(|e| error(shown, e.to_string()))
    }

    /// Writes `bytes` to a new temporary file beside `path`, creating parent
    /// directories. The temporary is created exclusively and never follows links.
    pub fn stage(&self, path: &RelPath, temp: &TempName, bytes: &[u8]) -> Result<(), TreeError> {
        let (dirs, _) = path.split();
        let dir = self.dir(&dirs, true)?.expect("created");
        Self::write_new(&dir, temp.as_str(), bytes, path.as_str())
    }

    /// Moves a staged temporary to `path` only if `path` does not exist.
    /// Returns `false`, leaving the temporary in place, if `path` exists.
    pub fn place_new(&self, path: &RelPath, temp: &TempName) -> Result<bool, TreeError> {
        let (dirs, name) = path.split();
        let dir = self
            .dir(&dirs, false)?
            .ok_or_else(|| error(path.as_str(), "parent directory vanished"))?;
        match rustix::fs::linkat(&dir, temp.as_str(), &dir, name, AtFlags::empty()) {
            Ok(()) => {}
            Err(Errno::EXIST) => return Ok(false),
            Err(e) => return Err(error(path.as_str(), describe(e))),
        }
        Self::unlink_ignoring_absent(&dir, temp.as_str(), path.as_str())?;
        Self::sync(&dir, path.as_str())?;
        Ok(true)
    }

    /// Atomically replaces `path` with a staged temporary.
    pub fn replace(&self, path: &RelPath, temp: &TempName) -> Result<(), TreeError> {
        let (dirs, name) = path.split();
        let dir = self
            .dir(&dirs, false)?
            .ok_or_else(|| error(path.as_str(), "parent directory vanished"))?;
        rustix::fs::renameat(&dir, temp.as_str(), &dir, name)
            .map_err(|e| error(path.as_str(), describe(e)))?;
        Self::sync(&dir, path.as_str())
    }

    /// Removes `path` (a file, never a directory); absent is not an error.
    pub fn remove(&self, path: &RelPath) -> Result<(), TreeError> {
        let (dirs, name) = path.split();
        if let Some(dir) = self.dir(&dirs, false)? {
            Self::unlink_ignoring_absent(&dir, name, path.as_str())?;
            Self::sync(&dir, path.as_str())?;
        }
        Ok(())
    }

    /// Removes a temporary beside `path`; absent is not an error.
    pub fn remove_temp(&self, path: &RelPath, temp: &TempName) -> Result<(), TreeError> {
        let (dirs, _) = path.split();
        if let Some(dir) = self.dir(&dirs, false)? {
            Self::unlink_ignoring_absent(&dir, temp.as_str(), path.as_str())?;
        }
        Ok(())
    }

    /// Atomically writes a root state file (via a temporary and rename).
    pub fn write_state(
        &self,
        name: &'static str,
        temp: &TempName,
        bytes: &[u8],
    ) -> Result<(), TreeError> {
        Self::write_new(&self.root, temp.as_str(), bytes, name)?;
        rustix::fs::renameat(&self.root, temp.as_str(), &self.root, name)
            .map_err(|e| error(name, describe(e)))?;
        Self::sync(&self.root, name)
    }

    /// Creates a root state file only if it does not exist yet. Returns `false`
    /// if it already exists. The file appears complete or not at all.
    pub fn create_state(
        &self,
        name: &'static str,
        temp: &TempName,
        bytes: &[u8],
    ) -> Result<bool, TreeError> {
        Self::write_new(&self.root, temp.as_str(), bytes, name)?;
        let linked = match rustix::fs::linkat(
            &self.root,
            temp.as_str(),
            &self.root,
            name,
            AtFlags::empty(),
        ) {
            Ok(()) => true,
            Err(Errno::EXIST) => false,
            Err(e) => return Err(error(name, describe(e))),
        };
        Self::unlink_ignoring_absent(&self.root, temp.as_str(), name)?;
        Self::sync(&self.root, name)?;
        Ok(linked)
    }

    /// Removes a root state file; absent is not an error.
    pub fn remove_state(&self, name: &'static str) -> Result<(), TreeError> {
        Self::unlink_ignoring_absent(&self.root, name, name)?;
        Self::sync(&self.root, name)
    }

    fn unlink_ignoring_absent(dir: &OwnedFd, name: &str, shown: &str) -> Result<(), TreeError> {
        match rustix::fs::unlinkat(dir, name, AtFlags::empty()) {
            Ok(()) | Err(Errno::NOENT) => Ok(()),
            Err(e) => Err(error(shown, describe(e))),
        }
    }

    fn sync(dir: &OwnedFd, shown: &str) -> Result<(), TreeError> {
        rustix::fs::fsync(dir)
            .map_err(|e| error(shown, format!("cannot sync directory: {}", describe(e))))
    }
}
