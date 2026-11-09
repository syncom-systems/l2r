//! Validated paths inside an output tree.
//!
//! Every path that L2R uses to touch an output tree must be a [`RelPath`]: paths
//! planned from the model, paths read back from the generation lock, and paths in
//! a recovery journal. The only way to obtain one is [`RelPath::parse`], so an
//! unvalidated string from any source cannot reach filesystem code.

use std::fmt;

use serde::{Deserialize, Serialize};

/// File names L2R reserves at the root of an output tree for its own state.
pub const RESERVED_NAMES: &[&str] = &["l2r.lock.json", "l2r.journal.json"];

const MAX_LEN: usize = 255;
const MAX_SEGMENTS: usize = 16;
const MAX_SEGMENT_LEN: usize = 100;

/// A relative path that cannot leave its base directory.
///
/// Guarantees: non-empty; `/`-separated; at most 16 segments and 255 bytes; each
/// segment non-empty, at most 100 bytes, made of `[A-Za-z0-9_.-]` and not
/// starting with `.` (so no `.`, `..`, hidden or temporary names); not absolute;
/// not one of the [`RESERVED_NAMES`], compared case-insensitively.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

/// Why a string is not a [`RelPath`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    /// The rejected input.
    pub input: String,
    /// The rule it broke.
    pub reason: &'static str,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsafe path `{}`: {}",
            self.input.escape_debug(),
            self.reason
        )
    }
}

impl std::error::Error for PathError {}

impl RelPath {
    /// Validates `input`.
    pub fn parse(input: &str) -> Result<RelPath, PathError> {
        let fail = |reason| {
            Err(PathError {
                input: input.to_string(),
                reason,
            })
        };
        if input.is_empty() {
            return fail("empty");
        }
        if input.len() > MAX_LEN {
            return fail("longer than 255 bytes");
        }
        if input.starts_with('/') {
            return fail("absolute");
        }
        let segments: Vec<&str> = input.split('/').collect();
        if segments.len() > MAX_SEGMENTS {
            return fail("more than 16 segments");
        }
        for segment in &segments {
            if segment.is_empty() {
                return fail("empty segment");
            }
            if segment.starts_with('.') {
                return fail("segment starts with `.`");
            }
            if segment.len() > MAX_SEGMENT_LEN {
                return fail("segment longer than 100 bytes");
            }
            if !segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
            {
                return fail("character outside [A-Za-z0-9_.-]");
            }
        }
        if RESERVED_NAMES.iter().any(|r| r.eq_ignore_ascii_case(input)) {
            return fail("reserved for L2R state");
        }
        Ok(RelPath(input.to_string()))
    }

    /// The path as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Directory segments (all but the last) and the file name.
    pub fn split(&self) -> (Vec<&str>, &str) {
        let mut segments: Vec<&str> = self.0.split('/').collect();
        let name = segments.pop().expect("a RelPath has at least one segment");
        (segments, name)
    }

    /// Key under which paths collide on case-insensitive filesystems.
    pub fn case_key(&self) -> String {
        self.0.to_ascii_lowercase()
    }
}

impl TryFrom<String> for RelPath {
    type Error = PathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        RelPath::parse(&value)
    }
}

impl From<RelPath> for String {
    fn from(path: RelPath) -> String {
        path.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for RelPath {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for RelPath {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_relative_paths() {
        for ok in ["BUILD.bazel", "src/lib.rs", "a/b-c/d_e.json", "x.y.z"] {
            assert!(RelPath::parse(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn rejects_every_escape_and_reserved_form() {
        let long = "a".repeat(101);
        let deep = vec!["a"; 17].join("/");
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a/./b",
            "./a",
            "a//b",
            "a/",
            ".hidden",
            "src/.mod.rs.l2r-tmp",
            "a\\b",
            "a b",
            "a\0b",
            "é",
            "l2r.lock.json",
            "L2R.LOCK.JSON",
            "l2r.journal.json",
            long.as_str(),
            deep.as_str(),
        ] {
            assert!(RelPath::parse(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn deserialization_validates() {
        assert!(serde_json::from_str::<RelPath>("\"../x\"").is_err());
        assert_eq!(
            serde_json::from_str::<RelPath>("\"src/lib.rs\"").unwrap(),
            "src/lib.rs"
        );
    }

    #[test]
    fn case_keys_identify_case_insensitive_collisions() {
        let a = RelPath::parse("Src/Lib.rs").unwrap();
        let b = RelPath::parse("src/lib.rs").unwrap();
        assert_ne!(a, b);
        assert_eq!(a.case_key(), b.case_key());
    }
}
