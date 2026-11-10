//! `l2r init`: the starter project, written under the new project's names.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use l2r_validate::names;

use crate::commands::Outcome;

/// The Lean toolchain the starter specification is written for.
const LEAN_TOOLCHAIN: &str = include_str!("../../../examples/starter/spec/lean-toolchain");

/// The starter project: a complete, valid `v2` system with one machine. The
/// same files are committed, generated and implemented, as `examples/starter`,
/// and the proof lane checks that the inventory is what its Lean exports.
const TEMPLATE: &[(&str, &str)] = &[
    (
        "architecture.toml",
        include_str!("../../../examples/starter/architecture.toml"),
    ),
    (
        "lean-inventory.json",
        include_str!("../../../examples/starter/lean-inventory.json"),
    ),
    (
        "spec/lakefile.lean",
        include_str!("../../../examples/starter/spec/lakefile.lean"),
    ),
    ("spec/lean-toolchain", LEAN_TOOLCHAIN),
    (
        "spec/Starter.lean",
        include_str!("../../../examples/starter/spec/Starter.lean"),
    ),
    (
        "spec/Starter/Monitor.lean",
        include_str!("../../../examples/starter/spec/Starter/Monitor.lean"),
    ),
    (
        "spec/Starter/Wiring.lean",
        include_str!("../../../examples/starter/spec/Starter/Wiring.lean"),
    ),
];

/// How a new workspace refers to L2R.
pub enum L2rSource {
    /// A commit of the public repository.
    Commit(String),
    /// A local checkout.
    Path(String),
}

impl L2rSource {
    /// The Lake `require` of L2R's Lean library.
    fn lake_requirement(&self) -> String {
        match self {
            L2rSource::Commit(commit) => format!(
                "require l2r from git \"https://github.com/syncom-systems/l2r.git\" @ \"{commit}\" / \"lean\""
            ),
            L2rSource::Path(path) => format!("require l2r from \"{path}\" / \"lean\""),
        }
    }

    /// The Bazel override that resolves the `l2r` module.
    fn bazel_override(&self) -> String {
        match self {
            L2rSource::Commit(commit) => format!(
                "git_override(\n    module_name = \"l2r\",\n    commit = \"{commit}\",\n    remote = \"https://github.com/syncom-systems/l2r.git\",\n)\n"
            ),
            L2rSource::Path(path) => {
                format!(
                    "local_path_override(\n    module_name = \"l2r\",\n    path = \"{path}\",\n)\n"
                )
            }
        }
    }
}

/// The project `l2r init` creates: the starter system under its own names.
pub struct NewProject {
    /// Directory to create the project in.
    pub dir: PathBuf,
    /// System identifier (lower snake case).
    pub id: String,
    /// Root Lean module and namespace (upper camel case).
    pub lean_root: String,
    /// When set, the project is also a new Bazel workspace.
    pub workspace: Option<L2rSource>,
}

/// Replaces `Starter` by `lean_root` and `starter` by `id`, in one pass, so
/// that replaced text is never scanned again.
fn instantiate(text: &str, project: &NewProject) -> String {
    let NewProject { id, lean_root, .. } = project;
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(tail) = rest.strip_prefix("Starter") {
            out.push_str(lean_root);
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("starter") {
            out.push_str(id);
            rest = tail;
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// `MonitorSystem` → `monitor_system` is not attempted: the default Lean root
/// is the identifier in upper camel case (`pump_station` → `PumpStation`).
pub fn default_lean_root(id: &str) -> Option<String> {
    names::upper_camel(id)
}

fn is_lean_root(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

fn build_file(project: &NewProject) -> String {
    let id = &project.id;
    format!(
        "load(\"@l2r//bazel:l2r.bzl\", \"l2r_check_test\", \"l2r_conformance_traces\")\n\
         \n\
         # Fails when the generated scaffold differs from the approved plan, when the\n\
         # approval is stale (the model, the Lean inventory, the generator or a profile\n\
         # changed since it was recorded), or when an implementation file exceeds its\n\
         # budget. Until a plan has been approved and generated it fails and says so.\n\
         l2r_check_test(name = \"scaffold_matches_approved_plan_test\")\n\
         \n\
         # The recorder of the derived conformance harness (`l2r conformance --rust`):\n\
         # exists once verification/traces.rs does. See verification/README.md.\n\
         l2r_conformance_traces(\n    name = \"conformance_traces\",\n    crate = \"{id}_scaffold\",\n)\n"
    )
}

/// The Lake `require` of L2R in a project whose workspace `init` did not write.
const UNPINNED_LAKE_REQUIREMENT: &str = "-- Pin this to the L2R commit your workspace uses.\n\
     require l2r from git \"https://github.com/syncom-systems/l2r.git\" @ \"main\" / \"lean\"";

fn verification_lakefile(project: &NewProject) -> String {
    let id = &project.id;
    let source = project.workspace.as_ref();
    let l2r = match source {
        Some(source) => source.lake_requirement(),
        None => UNPINNED_LAKE_REQUIREMENT.to_string(),
    };
    format!(
        "import Lake\nopen Lake DSL\n\n\
         /-!\nEvidence about the composed system: the network the architecture model approves\n\
         (derived by `l2r lean-composition`), the conformance replayer (derived by\n\
         `l2r conformance --lean`), and your own system proofs.\n-/\n\
         package «{id}_verification»\n\n\
         -- The specification, and L2R's semantics of composition.\n\
         require «{id}_spec» from \"..\" / \"spec\"\n\
         {l2r}\n\n\
         -- Every module under L2RDerived/ (there is no L2RDerived.lean).\n\
         @[default_target]\nlean_lib L2RDerived where\n  globs := #[.submodules `L2RDerived]\n"
    )
}

fn verification_readme(project: &NewProject) -> String {
    let id = &project.id;
    format!(
        "# Evidence about the composed system\n\
         \n\
         Nothing here is part of the generation plan; all of it is derived from the\n\
         model and the Lean inventory, or written by you about what was derived.\n\
         \n\
         ```sh\n\
         L2R=\"bazel run @l2r//crates/l2r_cli:l2r --\"\n\
         M=$PWD/architecture.toml\n\
         mkdir -p verification/L2RDerived\n\
         \n\
         # The network your model approves, as Lean, wired to L2R's semantics.\n\
         $L2R lean-composition $M > verification/L2RDerived/{id}.lean\n\
         \n\
         # Conformance: the generated Rust scheduler against its Lean model.\n\
         $L2R conformance $M --rust > verification/traces.rs\n\
         $L2R conformance $M --lean > verification/Conformance.lean\n\
         bazel run //:conformance_traces -- 3 500 40 > /tmp/{id}-traces.txt\n\
         (cd verification && lake build && lake env lean --run Conformance.lean /tmp/{id}-traces.txt)\n\
         ```\n\
         \n\
         The last line must print `CONFORMANCE n/n sequences agree`. The three numbers\n\
         are: the length up to which *every* operation sequence is tried, and the number\n\
         and length of pseudo-random longer ones.\n\
         \n\
         What this establishes, and what it does not:\n\
         \n\
         | Claim | Kind |\n\
         | --- | --- |\n\
         | Every run of the Lean scheduler model is a behaviour the semantics permit | proved, in L2R, for every network |\n\
         | The generated Rust `System` behaves like that model | **tested**, on the sequences and sample messages tried |\n\
         | `imp.rs` and `routes.rs` compute what the Lean specification says | **tested** by the same run, through the scheduler; not proved |\n\
         | Properties of the composed system | whatever you prove about `L2RDerived.{id}` (use `L2R.Async1.reachable_invariant`, then `L2R.Async1.Ports.run_invariant`) |\n\
         \n\
         Re-derive all three files after every change to the model or the Lean\n\
         specification: each records the digests of what it was derived from.\n"
    )
}

fn module_file(project: &NewProject, source: &L2rSource) -> String {
    let id = &project.id;
    let l2r_override = source.bazel_override();
    format!(
        "\"\"\"{id}: a system built with L2R (human-approved architecture, Lean behaviour, Rust implementation).\"\"\"\n\
         \n\
         module(name = \"{id}\", version = \"0.1.0\")\n\
         \n\
         # L2R: the `l2r` tool (`bazel run @l2r//crates/l2r_cli:l2r -- ...`) and the\n\
         # `l2r_check_test` rule. Pinned to one commit: the generator version is part\n\
         # of every plan digest, so changing it invalidates recorded approvals.\n\
         bazel_dep(name = \"l2r\", version = \"0.1.0\")\n\
         {l2r_override}\
         \n\
         # The generated scaffold is a `rust_library` with a `rust_test`.\n\
         bazel_dep(name = \"rules_rust\", version = \"0.74.0\")\n\
         bazel_dep(name = \"rules_shell\", version = \"0.6.1\")\n\
         \n\
         rust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\n\
         rust.toolchain(\n    edition = \"2024\",\n    versions = [\"1.97.1\"],\n)\n\
         use_repo(rust, \"rust_toolchains\")\n\
         \n\
         register_toolchains(\"@rust_toolchains//:all\")\n"
    )
}

fn readme(project: &NewProject) -> String {
    let NewProject { id, lean_root, .. } = project;
    format!(
        "# {id}\n\
         \n\
         Created by `l2r init`. Three sources of truth, each with one owner:\n\
         \n\
         | What | Where | Owner |\n\
         | --- | --- | --- |\n\
         | Behaviour: states, inputs, transitions, invariants, what messages mean | `spec/` (Lean 4) | you, in Lean |\n\
         | Structure: components, containment, ports, edges, capacities | `architecture.toml` | you, the architect |\n\
         | Implementation | `scaffold/**/imp.rs`, `routes.rs` | the implementer, inside the generated contracts |\n\
         \n\
         Everything else under `scaffold/` is generated and must not be edited.\n\
         \n\
         ## The loop\n\
         \n\
         ```sh\n\
         L2R=\"bazel run @l2r//crates/l2r_cli:l2r --\"\n\
         M=$PWD/architecture.toml\n\
         \n\
         # 1. Change behaviour in spec/{lean_root}/*.lean, then re-export the inventory\n\
         #    (needs Lean: https://lean-lang.org/install).\n\
         $L2R inventory $PWD/spec --out $PWD/lean-inventory.json {lean_root}\n\
         \n\
         # 2. Change structure in architecture.toml, then look at it.\n\
         $L2R validate $M\n\
         $L2R render $M            # Mermaid\n\
         \n\
         # 3. Review exactly what would be written, approve that plan, write it.\n\
         $L2R preview $M\n\
         $L2R approve $M --digest <digest from preview> --by \"<your name>\"\n\
         $L2R generate $M\n\
         \n\
         # 4. Implement. One work packet per component says what to edit and what it must satisfy.\n\
         $L2R packet $M monitor\n\
         \n\
         # 5. Test. The first test is the generated contract tests and your own;\n\
         #    the second fails on drift, a stale approval or a blown budget.\n\
         bazel test //...\n\
         \n\
         # 6. Evidence about the composed system: verification/README.md.\n\
         ```\n\
         \n\
         `generate` refuses to run when anything the approval was recorded for has\n\
         changed, and it never overwrites `imp.rs` or `routes.rs`.\n\
         \n\
         Passing Rust tests does not show that the implementation computes what the\n\
         Lean specification says. The L2R repository's `docs/getting-started.md` and\n\
         `docs/verification.md` describe the kinds of evidence and how to obtain them.\n"
    )
}

/// The files of a Bazel workspace around the project.
fn workspace_files(project: &NewProject, source: &L2rSource) -> [(String, String); 4] {
    [
        ("MODULE.bazel".into(), module_file(project, source)),
        (".bazelversion".into(), "9.2.0\n".into()),
        (
            ".bazelrc".into(),
            "common --enable_bzlmod\nbuild --incompatible_strict_action_env\ntest --test_output=errors\n".into(),
        ),
        (".gitignore".into(), "/bazel-*\n.lake/\n".into()),
    ]
}

/// Creates the file `target` with `text`, and its directory. Fails when the
/// file exists.
fn create_file(target: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    file.write_all(text.as_bytes())
}

impl NewProject {
    /// Says why the project's names cannot be used, if they cannot.
    fn check_names(&self) -> Result<(), String> {
        let Self { id, lean_root, .. } = self;
        if !names::is_identifier(id) || names::is_rust_keyword(id) {
            return Err(format!(
                "`{id}` is not a usable system id: use lower snake case, e.g. `pump_station`\n"
            ));
        }
        if !is_lean_root(lean_root) || names::upper_camel(lean_root).is_none() {
            return Err(format!(
                "`{lean_root}` is not a usable Lean root module: use upper camel case, e.g. `PumpStation`\n"
            ));
        }
        Ok(())
    }

    /// Every file of the project, as `(path, content)` in path order. The
    /// project is the root its README's paths are relative to.
    fn files(&self) -> Vec<(String, String)> {
        let mut files: Vec<(String, String)> = TEMPLATE
            .iter()
            .map(|(path, text)| (instantiate(path, self), instantiate(text, self)))
            .collect();
        files.push(("BUILD.bazel".into(), build_file(self)));
        files.push((
            "verification/lakefile.lean".into(),
            verification_lakefile(self),
        ));
        files.push(("verification/lean-toolchain".into(), LEAN_TOOLCHAIN.into()));
        files.push(("verification/README.md".into(), verification_readme(self)));
        files.push(("README.md".into(), readme(self)));
        if let Some(source) = &self.workspace {
            files.extend(workspace_files(self, source));
        }
        files.sort();
        files
    }

    /// Writes the project and reports what was created, or says why nothing
    /// (or, on an I/O error, not everything) was.
    fn create(&self) -> Result<String, String> {
        self.check_names()?;
        let dir = &self.dir;
        let files = self.files();
        // Fail closed: nothing is written if anything is already there.
        let existing: Vec<&str> = files
            .iter()
            .filter(|(path, _)| dir.join(path).symlink_metadata().is_ok())
            .map(|(path, _)| path.as_str())
            .collect();
        if !existing.is_empty() {
            return Err(format!(
                "init refused: {} already exists in {}; nothing was written\n",
                existing.join(", "),
                dir.display()
            ));
        }
        for (path, text) in &files {
            let target = dir.join(path);
            create_file(&target, text).map_err(|e| format!("{}: {e}\n", target.display()))?;
        }
        Ok(self.created(&files))
    }

    /// The report of a successful `init`.
    fn created(&self, files: &[(String, String)]) -> String {
        let dir = self.dir.display();
        let mut text = format!(
            "created project `{}` in {dir} ({} files)\n",
            self.id,
            files.len()
        );
        for (path, _) in files {
            text.push_str(&format!("  {path}\n"));
        }
        text.push_str(&format!(
            "\nNext: read {dir}/README.md. The model is valid as it stands:\n  l2r preview {dir}/architecture.toml\n"
        ));
        text
    }
}

/// `l2r init <dir> --id <id> [--lean-root <Name>] [--workspace ...]`
pub fn init(project: &NewProject) -> Outcome {
    match project.create() {
        Ok(created) => Outcome::ok(created),
        Err(refusal) => Outcome::refused(refusal),
    }
}

#[cfg(test)]
mod tests {
    use l2r_model::load;

    use super::*;

    fn project(id: &str, lean_root: &str) -> NewProject {
        NewProject {
            dir: PathBuf::new(),
            id: id.into(),
            lean_root: lean_root.into(),
            workspace: None,
        }
    }

    #[test]
    fn instantiation_replaces_both_names_in_one_pass() {
        assert_eq!(
            instantiate(
                "Starter.Monitor in starter_spec",
                &project("re_starter", "Restarter")
            ),
            "Restarter.Monitor in re_starter_spec"
        );
        assert_eq!(
            instantiate("no names here", &project("a", "A")),
            "no names here"
        );
    }

    #[test]
    fn lean_roots() {
        assert_eq!(
            default_lean_root("pump_station").as_deref(),
            Some("PumpStation")
        );
        assert!(is_lean_root("PumpStation"));
        for bad in ["pump", "Pump_Station", "Pump.Station", "", "1Pump"] {
            assert!(!is_lean_root(bad), "{bad}");
        }
    }

    #[test]
    fn the_template_is_a_valid_model_under_any_names() {
        let model = TEMPLATE[0].1;
        let inventory = TEMPLATE[1].1;
        let pump_station = project("pump_station", "PumpStation");
        let model = load::parse_model(&instantiate(model, &pump_station)).unwrap();
        let inventory = load::parse_inventory(&instantiate(inventory, &pump_station)).unwrap();
        // Renaming the root uniformly keeps the inventory canonical and the model valid.
        assert_eq!(inventory, inventory.canonical());
        let validated = l2r_validate::validate(&model, &inventory).expect("valid");
        assert_eq!(validated.model().system.id, "pump_station");
        assert!(validated.data_type("PumpStation.Monitor.State").is_some());
    }
}
