//! Hostile paths for the files L2R writes outside the output tree
//! (`l2r_apply::file`): planted symbolic links are replaced or refused, never
//! followed, and nothing outside the target is changed.

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;

use l2r_apply::file::{self, PrivateDir};
use l2r_testing::scratch_dir;

/// A file outside the project that a planted link points to.
fn victim(dir: &Path) -> std::path::PathBuf {
    let victim = dir.join("victim.txt");
    fs::write(&victim, "victim\n").unwrap();
    victim
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

#[test]
fn replace_writes_a_new_file_and_replaces_an_old_one() {
    let dir = scratch_dir("file-replace");
    let target = dir.join("lean-inventory.json");
    file::replace(&target, b"one\n").unwrap();
    file::replace(&target, b"two\n").unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "two\n");
    let left: Vec<_> = fs::read_dir(&dir).unwrap().collect();
    assert_eq!(left.len(), 1, "no staging file is left behind");
}

#[test]
fn replace_replaces_a_link_at_the_target_and_leaves_its_target_alone() {
    let dir = scratch_dir("file-link-target");
    let victim = victim(&dir);
    let target = dir.join("lean-inventory.json");
    symlink(&victim, &target).unwrap();
    file::replace(&target, b"inventory\n").unwrap();
    assert_eq!(fs::read_to_string(&victim).unwrap(), "victim\n");
    assert!(!is_link(&target), "the link was replaced by a regular file");
    assert_eq!(fs::read_to_string(&target).unwrap(), "inventory\n");
}

#[test]
fn replace_ignores_links_planted_at_predictable_staging_names() {
    // The names an earlier `l2r inventory` and `l2r approve` staged under.
    let dir = scratch_dir("file-link-staging");
    let victim = victim(&dir);
    let target = dir.join("lean-inventory.json");
    let planted = [
        dir.join("lean-inventory.json.tmp"),
        dir.join(format!(".lean-inventory.json.{}.tmp", std::process::id())),
    ];
    for link in &planted {
        symlink(&victim, link).unwrap();
    }
    file::replace(&target, b"inventory\n").unwrap();
    assert_eq!(fs::read_to_string(&victim).unwrap(), "victim\n");
    assert_eq!(fs::read_to_string(&target).unwrap(), "inventory\n");
    for link in &planted {
        assert!(is_link(link), "a planted link is left as it was");
    }
}

#[test]
fn create_refuses_anything_already_there_a_dangling_link_included() {
    let dir = scratch_dir("file-create");
    let victim = victim(&dir);
    let linked = dir.join("linked");
    symlink(&victim, &linked).unwrap();
    let dangling = dir.join("dangling");
    symlink(dir.join("absent"), &dangling).unwrap();
    for target in [&linked, &dangling, &victim] {
        let refused = file::create(target, b"new\n").unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::AlreadyExists);
    }
    assert_eq!(fs::read_to_string(&victim).unwrap(), "victim\n");
    assert!(!dir.join("absent").exists());
    file::create(&dir.join("fresh"), b"new\n").unwrap();
    assert_eq!(fs::read_to_string(dir.join("fresh")).unwrap(), "new\n");
}

#[test]
fn a_private_dir_is_fresh_owner_only_and_removed_when_dropped() {
    let parent = scratch_dir("file-private");
    let path = {
        let dir = PrivateDir::create_in(&parent).unwrap();
        let mode = fs::metadata(dir.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        let exporter = dir
            .create_file("Exporter.lean", b"def main : IO Unit := pure ()\n")
            .unwrap();
        assert_eq!(exporter.parent(), Some(dir.path()));
        for bad in ["", ".", "..", "../escape", "a/b"] {
            assert!(dir.create_file(bad, b"x").is_err(), "{bad:?} is refused");
        }
        assert!(dir.create_file("Exporter.lean", b"again").is_err());
        dir.path().to_path_buf()
    };
    assert!(!path.exists());
    assert!(!parent.join("escape").exists());
}

#[test]
fn private_dirs_never_reuse_a_name() {
    let parent = scratch_dir("file-private-names");
    let first = PrivateDir::create_in(&parent).unwrap();
    let second = PrivateDir::create_in(&parent).unwrap();
    assert_ne!(first.path(), second.path());
}
