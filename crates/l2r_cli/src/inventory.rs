//! `l2r inventory`: the declaration inventory a Lake project exports.

use std::path::Path;
use std::process::{Command, Output};

use l2r_apply::file::{self, PrivateDir};
use l2r_model::{Inventory, load};
use l2r_validate::names;

use crate::commands::Outcome;

/// The Lean exporter, run by `l2r inventory`.
const EXPORTER: &str = include_str!("../../../tools/lean_inventory/Main.lean");

/// `l2r inventory <lake-project-dir> --out <file> [--v0] <RootModule>...`
///
/// Builds the Lake project and exports the declaration inventory of its
/// compiled modules. Runs `lake`, which must be on `PATH`.
pub fn inventory(project: &Path, out: &Path, v0: bool, roots: &[String]) -> Outcome {
    match replace_inventory(project, out, v0, roots) {
        Ok(written) => Outcome::ok(written),
        Err(refusal) => Outcome::refused(refusal),
    }
}

/// Exports the inventory of `project` and puts it at `out`. Reports what was
/// written, or says why `out` was left as it was.
fn replace_inventory(
    project: &Path,
    out: &Path,
    v0: bool,
    roots: &[String],
) -> Result<String, String> {
    if roots.is_empty() || !roots.iter().all(|r| names::is_lean_name(r)) {
        return Err(
            "`inventory` needs the root Lean modules to export, e.g. `PumpStation`\n".into(),
        );
    }
    if !is_lake_project(project) {
        return Err(format!(
            "{} is not a Lake project (no lakefile.lean or lakefile.toml)\n",
            project.display()
        ));
    }
    let text = export(project, v0, roots)?;
    // Only a well-formed inventory replaces the previous one.
    let parsed: Inventory = load::parse_inventory(&text)
        .map_err(|e| format!("the exporter's output is not an inventory: {e}\n"))?;
    file::replace(out, text.as_bytes()).map_err(|e| format!("{}: {e}\n", out.display()))?;
    Ok(format!(
        "wrote {} ({}, {} declarations, Lean {})\n\
         the inventory's digest is part of every plan: preview and approve again before generating\n",
        out.display(),
        parsed.schema,
        parsed.declarations.len(),
        parsed.lean_version
    ))
}

fn is_lake_project(dir: &Path) -> bool {
    dir.join("lakefile.lean").is_file() || dir.join("lakefile.toml").is_file()
}

/// Builds the Lake project and returns what the exporter prints for `roots`.
/// The exporter is written to a fresh private directory, so no other user can
/// replace the program `lean` runs; the directory is removed afterwards.
fn export(project: &Path, v0: bool, roots: &[String]) -> Result<String, String> {
    let temp = std::env::temp_dir();
    let dir = PrivateDir::create_in(&temp).map_err(|e| format!("{}: {e}\n", temp.display()))?;
    let exporter = dir
        .create_file("Exporter.lean", EXPORTER.as_bytes())
        .map_err(|e| format!("{}: {e}\n", dir.path().display()))?;
    run_exporter(project, &exporter, v0, roots)
}

/// Runs `lake` with `args` in `project`.
fn lake(project: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new("lake")
        .args(args)
        .current_dir(project)
        .output()
        .map_err(|e| {
            format!(
                "cannot run `lake` ({e}); install Lean (https://lean-lang.org/install) so that `lake` is on PATH\n"
            )
        })
}

fn run_exporter(
    project: &Path,
    exporter: &Path,
    v0: bool,
    roots: &[String],
) -> Result<String, String> {
    let build = lake(project, &["build"])?;
    if !build.status.success() {
        return Err(format!(
            "`lake build` failed in {}:\n{}{}",
            project.display(),
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        ));
    }
    let exporter = exporter.display().to_string();
    let mut args = vec!["env", "lean", "--run", exporter.as_str()];
    if !v0 {
        args.push("--v1");
    }
    args.extend(roots.iter().map(String::as_str));
    let export = lake(project, &args)?;
    if !export.status.success() {
        return Err(format!(
            "exporting the inventory failed:\n{}",
            String::from_utf8_lossy(&export.stderr)
        ));
    }
    String::from_utf8(export.stdout)
        .map_err(|e| format!("the exporter's output is not UTF-8: {e}\n"))
}
