//! Commands that start and support a project: `init`, `inventory`, `packet`.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use l2r_model::{Component, Inventory, load};
use l2r_plan::Ownership;
use l2r_validate::names;

use crate::commands::Outcome;
use crate::session::{self, Session, SessionError};

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

/// The Lean exporter, run by `l2r inventory`.
const EXPORTER: &str = include_str!("../../../tools/lean_inventory/Main.lean");

/// How a new workspace refers to L2R.
pub enum L2rSource {
    /// A commit of the public repository.
    Commit(String),
    /// A local checkout.
    Path(String),
}

/// Options of `l2r init`.
pub struct InitOptions {
    /// Directory to create the project in.
    pub dir: PathBuf,
    /// System identifier (lower snake case).
    pub id: String,
    /// Root Lean module and namespace (upper camel case).
    pub lean_root: String,
    /// When set, also write the files of a new Bazel workspace.
    pub workspace: Option<L2rSource>,
}

/// Replaces `Starter` by `lean_root` and `starter` by `id`, in one pass, so
/// that replaced text is never scanned again.
fn instantiate(text: &str, id: &str, lean_root: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix("Starter") {
            out.push_str(lean_root);
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("starter") {
            out.push_str(id);
            rest = tail;
        } else {
            let mut chars = rest.chars();
            if let Some(c) = chars.next() {
                out.push(c);
            }
            rest = chars.as_str();
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

fn build_file() -> String {
    "load(\"@l2r//bazel:l2r.bzl\", \"l2r_check_test\")\n\
     \n\
     # Fails when the generated scaffold differs from the approved plan, when the\n\
     # approval is stale (the model, the Lean inventory, the generator or a profile\n\
     # changed since it was recorded), or when an implementation file exceeds its\n\
     # budget. Until a plan has been approved and generated it fails and says so.\n\
     l2r_check_test(name = \"scaffold_matches_approved_plan_test\")\n"
        .to_string()
}

fn module_file(id: &str, source: &L2rSource) -> String {
    let l2r_override = match source {
        L2rSource::Commit(commit) => format!(
            "git_override(\n    module_name = \"l2r\",\n    commit = \"{commit}\",\n    remote = \"https://github.com/syncom-systems/l2r.git\",\n)\n"
        ),
        L2rSource::Path(path) => {
            format!("local_path_override(\n    module_name = \"l2r\",\n    path = \"{path}\",\n)\n")
        }
    };
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

fn readme(id: &str, lean_root: &str, project: &str) -> String {
    format!(
        "# {id}\n\
         \n\
         Created by `l2r init`. Three sources of truth, each with one owner:\n\
         \n\
         | What | Where | Owner |\n\
         | --- | --- | --- |\n\
         | Behaviour: states, inputs, transitions, invariants, what messages mean | `{project}spec/` (Lean 4) | you, in Lean |\n\
         | Structure: components, containment, ports, edges, capacities | `{project}architecture.toml` | you, the architect |\n\
         | Implementation | `{project}scaffold/**/imp.rs`, `routes.rs` | the implementer, inside the generated contracts |\n\
         \n\
         Everything else under `{project}scaffold/` is generated and must not be edited.\n\
         \n\
         ## The loop\n\
         \n\
         ```sh\n\
         L2R=\"bazel run @l2r//crates/l2r_cli:l2r --\"\n\
         M=$PWD/{project}architecture.toml\n\
         \n\
         # 1. Change behaviour in {project}spec/{lean_root}/*.lean, then re-export the inventory\n\
         #    (needs Lean: https://lean-lang.org/install).\n\
         $L2R inventory $PWD/{project}spec --out $PWD/{project}lean-inventory.json {lean_root}\n\
         \n\
         # 2. Change structure in {project}architecture.toml, then look at it.\n\
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

/// `l2r init <dir> --id <id> [--lean-root <Name>] [--workspace ...]`
pub fn init(options: &InitOptions) -> Outcome {
    let InitOptions {
        dir,
        id,
        lean_root,
        workspace,
    } = options;
    if !names::is_identifier(id) || names::is_rust_keyword(id) {
        return Outcome::refused(format!(
            "`{id}` is not a usable system id: use lower snake case, e.g. `pump_station`\n"
        ));
    }
    if !is_lean_root(lean_root) || names::upper_camel(lean_root).is_none() {
        return Outcome::refused(format!(
            "`{lean_root}` is not a usable Lean root module: use upper camel case, e.g. `PumpStation`\n"
        ));
    }
    // In a new workspace the project is the repository root; otherwise the
    // README's paths are relative to wherever `dir` sits.
    let mut files: Vec<(String, String)> = TEMPLATE
        .iter()
        .map(|(path, text)| {
            (
                instantiate(path, id, lean_root),
                instantiate(text, id, lean_root),
            )
        })
        .collect();
    files.push(("BUILD.bazel".into(), build_file()));
    files.push(("README.md".into(), readme(id, lean_root, "")));
    if let Some(source) = workspace {
        files.push(("MODULE.bazel".into(), module_file(id, source)));
        files.push((".bazelversion".into(), "9.2.0\n".into()));
        files.push((
            ".bazelrc".into(),
            "common --enable_bzlmod\nbuild --incompatible_strict_action_env\ntest --test_output=errors\n".into(),
        ));
        files.push((".gitignore".into(), "/bazel-*\n.lake/\n".into()));
    }
    files.sort();

    // Fail closed: nothing is written if anything is already there.
    let existing: Vec<&str> = files
        .iter()
        .filter(|(path, _)| dir.join(path).symlink_metadata().is_ok())
        .map(|(path, _)| path.as_str())
        .collect();
    if !existing.is_empty() {
        return Outcome::refused(format!(
            "init refused: {} already exists in {}; nothing was written\n",
            existing.join(", "),
            dir.display()
        ));
    }
    for (path, text) in &files {
        let target = dir.join(path);
        let written = target
            .parent()
            .map(fs::create_dir_all)
            .unwrap_or(Ok(()))
            .and_then(|()| {
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
            })
            .and_then(|mut file| file.write_all(text.as_bytes()));
        if let Err(e) = written {
            return Outcome::refused(format!("{}: {e}\n", target.display()));
        }
    }
    let mut text = format!(
        "created project `{id}` in {} ({} files)\n",
        dir.display(),
        files.len()
    );
    for (path, _) in &files {
        text.push_str(&format!("  {path}\n"));
    }
    text.push_str(&format!(
        "\nNext: read {}/README.md. The model is valid as it stands:\n  l2r preview {}/architecture.toml\n",
        dir.display(),
        dir.display()
    ));
    Outcome::ok(text)
}

/// `l2r inventory <lake-project-dir> --out <file> [--v0] <RootModule>...`
///
/// Builds the Lake project and exports the declaration inventory of its
/// compiled modules. Runs `lake`, which must be on `PATH`.
pub fn inventory(project: &Path, out: &Path, v0: bool, roots: &[String]) -> Outcome {
    if roots.is_empty() || !roots.iter().all(|r| names::is_lean_name(r)) {
        return Outcome::refused(
            "`inventory` needs the root Lean modules to export, e.g. `PumpStation`\n".into(),
        );
    }
    if !project.join("lakefile.lean").is_file() && !project.join("lakefile.toml").is_file() {
        return Outcome::refused(format!(
            "{} is not a Lake project (no lakefile.lean or lakefile.toml)\n",
            project.display()
        ));
    }
    let exporter = std::env::temp_dir().join(format!("l2r-inventory-{}.lean", std::process::id()));
    if let Err(e) = fs::write(&exporter, EXPORTER) {
        return Outcome::refused(format!("{}: {e}\n", exporter.display()));
    }
    let result = run_exporter(project, &exporter, v0, roots);
    let _ = fs::remove_file(&exporter);
    let text = match result {
        Ok(text) => text,
        Err(message) => return Outcome::refused(message),
    };
    // Only a well-formed inventory replaces the previous one.
    let parsed: Inventory = match load::parse_inventory(&text) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Outcome::refused(format!("the exporter's output is not an inventory: {e}\n"));
        }
    };
    let staged = out.with_extension("json.tmp");
    if let Err(e) = fs::write(&staged, &text).and_then(|()| fs::rename(&staged, out)) {
        return Outcome::refused(format!("{}: {e}\n", out.display()));
    }
    Outcome::ok(format!(
        "wrote {} ({}, {} declarations, Lean {})\n\
         the inventory's digest is part of every plan: preview and approve again before generating\n",
        out.display(),
        parsed.schema,
        parsed.declarations.len(),
        parsed.lean_version
    ))
}

fn run_exporter(
    project: &Path,
    exporter: &Path,
    v0: bool,
    roots: &[String],
) -> Result<String, String> {
    let lake = |args: &[&str]| {
        Command::new("lake")
            .args(args)
            .current_dir(project)
            .output()
            .map_err(|e| {
                format!(
                    "cannot run `lake` ({e}); install Lean (https://lean-lang.org/install) so that `lake` is on PATH\n"
                )
            })
    };
    let build = lake(&["build"])?;
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
    let export = lake(&args)?;
    if !export.status.success() {
        return Err(format!(
            "exporting the inventory failed:\n{}",
            String::from_utf8_lossy(&export.stderr)
        ));
    }
    String::from_utf8(export.stdout)
        .map_err(|e| format!("the exporter's output is not UTF-8: {e}\n"))
}

/// `l2r packet <model> <component>`: everything an implementer (human or
/// agent) needs to work on one component, and nothing that invites changing
/// the architecture.
pub fn packet(model: &Path, component_id: &str) -> Outcome {
    let session = match Session::open(model) {
        Ok(session) => session,
        Err(SessionError::Load(message) | SessionError::Plan(message)) => {
            return Outcome::refused(format!("{message}\n"));
        }
        Err(SessionError::Invalid(diagnostics)) => {
            let mut text: String = diagnostics.iter().map(|d| format!("{d}\n")).collect();
            text.push_str("model rejected: a work packet is issued only for a valid model\n");
            return Outcome::refused(text);
        }
    };
    let model_data = session.model();
    let Some(component) = model_data.component(component_id) else {
        let ids: Vec<&str> = model_data
            .components
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        return Outcome::refused(format!(
            "no component `{component_id}`; components: {}\n",
            ids.join(", ")
        ));
    };
    Outcome::ok(packet_text(&session, component))
}

fn packet_text(session: &Session, component: &Component) -> String {
    let model = session.model();
    let inventory = session.validated.inventory();
    let plan = &session.plan;
    let out_dir = &plan.output_dir;
    let type_of = |name: &str| {
        inventory
            .find(name)
            .map(|d| d.type_.clone())
            .unwrap_or_else(|| "(not in the inventory)".into())
    };
    let owned: Vec<_> = plan
        .artifacts
        .iter()
        .filter(|a| a.components.first() == Some(&component.id))
        .collect();

    let mut out = format!(
        "# Work packet: {} (`{}`)\n\n\
         System `{}`, profile `{}`{}. Plan digest `{}`.\n\n\
         ## Responsibility\n\n{}\n",
        component.name,
        component.id,
        model.system.id,
        component.profile,
        component
            .parent
            .as_ref()
            .map(|p| format!(", inside `{p}`"))
            .unwrap_or_default(),
        plan.digest(),
        component.responsibility
    );
    if let Some(notes) = &component.notes {
        out.push_str(&format!("\n## Architect's notes\n\n{notes}\n"));
    }

    out.push_str("\n## Files you may edit\n\n");
    let editable: Vec<_> = owned
        .iter()
        .filter(|a| a.ownership == Ownership::Implementation)
        .collect();
    if editable.is_empty() {
        out.push_str("None. This component has no implementation file.\n");
    }
    for artifact in &editable {
        out.push_str(&format!(
            "- `{out_dir}/{}`: {}\n",
            artifact.path, artifact.purpose
        ));
    }
    if let Some(budget) = component.implementation_budget_lines {
        out.push_str(&format!(
            "\nSize budget: at most {budget} lines per implementation file (`l2r check` enforces it).\n"
        ));
    }

    out.push_str("\n## Contract: read, never edit\n\n");
    for artifact in owned.iter().filter(|a| a.ownership == Ownership::Generated) {
        out.push_str(&format!(
            "- `{out_dir}/{}`: {}\n",
            artifact.path, artifact.purpose
        ));
    }
    if plan.artifact("src/types.rs").is_some() {
        out.push_str(&format!(
            "- `{out_dir}/src/types.rs`: the Rust mirror of every Lean data type\n"
        ));
    }

    if let Some(behavior) = &component.behavior {
        out.push_str(&format!(
            "\n## Behaviour to implement (Lean is authoritative)\n\n\
             | Role | Lean declaration | Type |\n| --- | --- | --- |\n\
             | owned state | `{}` | |\n| input | `{}` | |\n| output | `{}` | |\n\
             | initial state | `{}` | `{}` |\n| transition | `{}` | `{}` |\n",
            behavior.owned_state,
            behavior.input,
            behavior.output,
            behavior.init,
            type_of(&behavior.init),
            behavior.step,
            type_of(&behavior.step),
        ));
        if let Some(capacity) = component.mailbox_capacity {
            out.push_str(&format!("\nMailbox capacity: {capacity} messages.\n"));
        }
        out.push_str(
            "\n## Theorems about the specification\n\n\
             Proved in Lean about the specification. The implementation inherits them only\n\
             if it computes exactly what the transition says.\n\n",
        );
        if behavior.invariants.is_empty() {
            out.push_str("(None referenced.)\n");
        }
        for invariant in &behavior.invariants {
            out.push_str(&format!("- `{invariant}`: `{}`\n", type_of(invariant)));
        }
    }

    // Translations this component's files implement.
    let mut translations: Vec<(String, String, String)> = Vec::new();
    if let Some(flat) = session.validated.flat() {
        let mut seen = std::collections::BTreeSet::new();
        for hop in flat
            .leaves
            .iter()
            .flat_map(|l| l.routes.iter().flat_map(|r| r.hops.iter()))
            .chain(flat.inputs.iter().flat_map(|i| i.hops.iter()))
            .filter(|h| h.owner == component.id)
        {
            let function = match hop.kind {
                l2r_validate::flat::HopKind::Edge => hop.id.clone(),
                l2r_validate::flat::HopKind::Port => format!("port_{}", hop.id),
            };
            if seen.insert(function.clone()) {
                translations.push((function, hop.translation.clone(), type_of(&hop.translation)));
            }
        }
    } else {
        for edge in &model.messages {
            let parent = model
                .component(&edge.from)
                .and_then(|c| c.parent.as_deref());
            if parent == Some(component.id.as_str()) {
                translations.push((
                    edge.id.clone(),
                    edge.translation.clone(),
                    type_of(&edge.translation),
                ));
            }
        }
        for port in model.ports.iter().filter(|p| p.composite == component.id) {
            translations.push((
                format!("port_{}", port.id),
                port.translation_name().to_string(),
                type_of(port.translation_name()),
            ));
        }
    }
    if !translations.is_empty() {
        translations.sort();
        out.push_str(
            "\n## Translations to implement (in `routes.rs`)\n\n\
             | Function | Lean declaration | Type |\n| --- | --- | --- |\n",
        );
        for (function, lean, ty) in &translations {
            out.push_str(&format!("| `{function}` | `{lean}` | `{ty}` |\n"));
        }
    }

    // Where the component sits in the message structure.
    let mut connections = String::new();
    for edge in &model.messages {
        if edge.from == component.id || edge.to == component.id {
            connections.push_str(&format!(
                "- edge `{}`: `{}` → `{}` through `{}`\n",
                edge.id, edge.from, edge.to, edge.translation
            ));
        }
    }
    for port in &model.ports {
        if port.composite == component.id || port.target() == component.id {
            let direction = if port.is_out() { "out" } else { "in" };
            let target = match port.target() {
                "" => String::new(),
                target => format!(" → `{target}` through `{}`", port.translation_name()),
            };
            connections.push_str(&format!(
                "- {direction} port `{}` of `{}`: `{}`{target}\n",
                port.id, port.composite, port.message
            ));
        }
    }
    if !connections.is_empty() {
        out.push_str("\n## Approved connections\n\n");
        out.push_str(&connections);
    }

    out.push_str(
        "\n## Rules\n\n\
         - Edit only the files listed under \"Files you may edit\". Generated files are\n  \
           rewritten from the model; `l2r check` reports any edit to them as drift.\n\
         - Do not add state, I/O, clocks, public items or dependencies. The component's\n  \
           state is exactly its Lean state type; only its own transition changes it.\n\
         - Implement what the Lean declarations say, including integer arithmetic\n  \
           (Lean's fixed-width integers wrap; Rust's `+` does not).\n\
         - If the contract cannot be satisfied, or the specification looks wrong, stop\n  \
           and report it. Do not change the model, the Lean specification, the tests or\n  \
           the approval record to make something pass: those belong to the architect.\n\
         - Passing Rust tests is not evidence of agreement with Lean; say which evidence\n  \
           you have (tests, conformance run, refinement proof) and which you do not.\n",
    );
    let _ = session::approval_path;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instantiation_replaces_both_names_in_one_pass() {
        assert_eq!(
            instantiate("Starter.Monitor in starter_spec", "re_starter", "Restarter"),
            "Restarter.Monitor in re_starter_spec"
        );
        assert_eq!(instantiate("no names here", "a", "A"), "no names here");
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
        let model = load::parse_model(&instantiate(model, "pump_station", "PumpStation")).unwrap();
        let inventory =
            load::parse_inventory(&instantiate(inventory, "pump_station", "PumpStation")).unwrap();
        // Renaming the root uniformly keeps the inventory canonical and the model valid.
        assert_eq!(inventory, inventory.canonical());
        let validated = l2r_validate::validate(&model, &inventory).expect("valid");
        assert_eq!(validated.model().system.id, "pump_station");
        assert!(validated.data_type("PumpStation.Monitor.State").is_some());
    }
}
