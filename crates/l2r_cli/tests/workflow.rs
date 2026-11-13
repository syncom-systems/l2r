//! The architect's workflow through the real `l2r` binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use l2r_testing::{EXAMPLE_INVENTORY, EXAMPLE_MODEL, scratch_dir};

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn l2r(args: &[&str]) -> Run {
    let bin = std::env::var("L2R_BIN").expect("L2R_BIN is set by Bazel");
    let out = Command::new(bin).args(args).output().expect("l2r runs");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn project(label: &str) -> (PathBuf, String) {
    let dir = scratch_dir(label);
    fs::write(dir.join("architecture.toml"), EXAMPLE_MODEL).unwrap();
    fs::write(dir.join("lean-inventory.json"), EXAMPLE_INVENTORY).unwrap();
    let model = dir.join("architecture.toml").display().to_string();
    (dir, model)
}

fn digest_from(plan_output: &str) -> String {
    plan_output
        .lines()
        .find_map(|l| l.strip_prefix("Plan digest: "))
        .expect("digest printed")
        .to_string()
}

fn approve(model: &str, digest: &str) -> Run {
    l2r(&[
        "approve",
        model,
        "--digest",
        digest,
        "--by",
        "Test Architect",
    ])
}

#[test]
fn validate_plan_preview_approve_generate_check() {
    let (dir, model) = project("workflow");
    let validated = l2r(&["validate", &model]);
    assert_eq!(validated.code, 0, "{}", validated.stderr);

    let plan = l2r(&["plan", &model]);
    assert_eq!(plan.code, 0);
    let digest = digest_from(&plan.stdout);

    let preview = l2r(&["preview", &model]);
    assert!(
        preview
            .stdout
            .contains("create                        src/crossing/signal/imp.rs"),
        "{}",
        preview.stdout
    );
    assert!(
        preview.stdout.contains("+pub enum State {"),
        "preview shows exact content"
    );
    assert!(!dir.join("scaffold").exists(), "preview writes nothing");

    let refused = l2r(&["generate", &model]);
    assert_eq!(refused.code, 1);
    assert!(
        refused.stderr.contains("no approval recorded"),
        "{}",
        refused.stderr
    );
    assert!(!dir.join("scaffold").exists());

    let approval = approve(&model, &digest);
    assert_eq!(approval.code, 0, "{}", approval.stderr);
    let generated = l2r(&["generate", &model]);
    assert_eq!(generated.code, 0, "{}", generated.stderr);
    assert!(dir.join("scaffold/src/crossing/signal/mod.rs").exists());

    let checked = l2r(&["check", &model]);
    assert_eq!(checked.code, 0, "{}", checked.stderr);
}

#[test]
fn approving_a_digest_that_was_not_shown_is_refused() {
    let (dir, model) = project("wrong-digest");
    let run = approve(&model, "sha256:0000");
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("approval refused"), "{}", run.stderr);
    assert!(!dir.join("architecture.approval.json").exists());
}

#[test]
fn a_model_edit_after_approval_blocks_generation() {
    let (dir, model) = project("stale");
    let digest = digest_from(&l2r(&["plan", &model]).stdout);
    assert_eq!(approve(&model, &digest).code, 0);
    let edited = EXAMPLE_MODEL.replace("Owns no behavioral state of its own.", "Owns nothing.");
    fs::write(dir.join("architecture.toml"), edited).unwrap();

    let run = l2r(&["generate", &model]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("the architecture model changed"),
        "{}",
        run.stderr
    );
    assert!(!dir.join("scaffold").exists());
    let check = l2r(&["check", &model]);
    assert!(
        check.stderr.contains("approval is stale"),
        "{}",
        check.stderr
    );
}

#[test]
fn invalid_models_are_rejected_with_every_finding() {
    let (dir, model) = project("invalid");
    let broken = EXAMPLE_MODEL
        .replace(
            "rust_module = \"signal\"",
            "rust_module = \"request_latch\"",
        )
        .replace("Crossing.Latch.step", "Crossing.Latch.stepp");
    fs::write(dir.join("architecture.toml"), broken).unwrap();
    let run = l2r(&["validate", &model]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("L2R-S009") && run.stderr.contains("L2R-L003"),
        "{}",
        run.stderr
    );
}

#[test]
fn unknown_model_fields_are_rejected() {
    let (dir, model) = project("unknown-field");
    fs::write(
        dir.join("architecture.toml"),
        EXAMPLE_MODEL.replace("responsibility = \"Cycles", "responsiblity = \"Cycles"),
    )
    .unwrap();
    let run = l2r(&["validate", &model]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("unknown field `responsiblity`"),
        "{}",
        run.stderr
    );
}

#[test]
fn an_escaping_output_directory_is_rejected_before_any_file_access() {
    let (dir, model) = project("escape");
    fs::write(
        dir.join("architecture.toml"),
        EXAMPLE_MODEL.replace("output_dir = \"scaffold\"", "output_dir = \"../outside\""),
    )
    .unwrap();
    let run = l2r(&["plan", &model]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("L2R-S013"), "{}", run.stderr);
    assert!(!Path::new(&dir).join("../outside").exists());
}

#[test]
fn render_shows_containment_ownership_and_edges() {
    let (_dir, model) = project("render");
    let run = l2r(&["render", &model]);
    assert_eq!(run.code, 0);
    assert!(run.stdout.contains("subgraph crossing"));
    assert!(run.stdout.contains("owns Crossing.Signal.State"));
    assert!(
        run.stdout
            .contains("request_latch -- \"latch_to_signal\" --> signal")
    );
}

#[test]
fn lean_composition_derives_the_network_without_touching_the_plan() {
    let (dir, model) = project("lean-composition");
    let before = l2r(&["plan", &model]).stdout;
    let run = l2r(&["lean-composition", &model]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout
            .starts_with("-- @generated by L2R (l2r.lean-composition/v0)")
    );
    assert!(
        run.stdout
            .contains("namespace L2RDerived.«crossing».«crossing»")
    );
    assert!(run.stdout.contains("| ⟨1, _⟩ => Crossing.Signal.step"));
    // Deriving writes nothing and leaves the plan (and so its digest) unchanged.
    assert_eq!(l2r(&["plan", &model]).stdout, before);
    assert!(!dir.join("scaffold").exists());
}

#[test]
fn json_plans_are_byte_identical_across_runs() {
    let (_dir, model) = project("json");
    let a = l2r(&["plan", &model, "--json"]);
    let b = l2r(&["plan", &model, "--json"]);
    assert_eq!(a.code, 0);
    assert_eq!(a.stdout, b.stdout);
    assert!(a.stdout.contains("\"content_digest\""));
}

#[test]
fn a_responsibility_edit_shows_and_records_the_replacement_it_needs() {
    // The security argument rests on the reviewer seeing what will be destroyed.
    let (dir, model) = project("destructive");
    let digest = digest_from(&l2r(&["plan", &model]).stdout);
    assert_eq!(approve(&model, &digest).code, 0);
    assert_eq!(l2r(&["generate", &model]).code, 0);
    let mod_rs = dir.join("scaffold/src/crossing/signal/mod.rs");
    let before = l2r_model::digest::sha256(&fs::read(&mod_rs).unwrap());

    fs::write(
        dir.join("architecture.toml"),
        EXAMPLE_MODEL.replace("Cycles vehicle", "Steps vehicle"),
    )
    .unwrap();
    let preview = l2r(&["preview", &model]);
    assert!(
        preview
            .stdout
            .contains("permission to destroy existing content"),
        "{}",
        preview.stdout
    );
    assert!(
        preview.stdout.contains(&format!(
            "Replace src/crossing/signal/mod.rs while it is {before}"
        )),
        "{}",
        preview.stdout
    );

    // Without approving again, generation refuses and says why.
    let refused = l2r(&["generate", &model]);
    assert_eq!(refused.code, 1);

    let next = digest_from(&l2r(&["plan", &model]).stdout);
    let approved = approve(&model, &next);
    assert_eq!(approved.code, 0, "{}", approved.stderr);
    assert!(approved.stdout.contains("authorizes replacing or deleting"));
    let record: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("architecture.approval.json")).unwrap())
            .unwrap();
    let changes = record["authorized_changes"].as_array().unwrap();
    assert!(
        changes
            .iter()
            .any(|c| c["path"] == "src/crossing/signal/mod.rs"
                && c["change"] == "replace"
                && c["before"] == before.as_str()),
        "{changes:?}"
    );
    assert_eq!(l2r(&["generate", &model]).code, 0);
}

#[test]
fn render_shows_a_v1_composite_with_its_capacities_and_ports() {
    let dir = scratch_dir("render-v1");
    fs::write(dir.join("architecture.toml"), l2r_testing::EXAMPLE_V1_MODEL).unwrap();
    fs::write(
        dir.join("lean-inventory.json"),
        l2r_testing::EXAMPLE_V1_INVENTORY,
    )
    .unwrap();
    let run = l2r(&[
        "render",
        &dir.join("architecture.toml").display().to_string(),
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.contains("subgraph crossing"), "{}", run.stdout);
    assert!(run.stdout.contains("mailbox 2"), "{}", run.stdout);
    assert!(
        run.stdout
            .contains("env_button([\"port button: CrossingV1.Ports.ButtonEvent\"]) -- \"button\" --> request_latch"),
        "{}",
        run.stdout
    );
}

#[test]
fn init_creates_a_valid_project_and_never_overwrites() {
    let dir = scratch_dir("init").join("pump");
    let target = dir.display().to_string();
    let created = l2r(&["init", &target, "--id", "pump_station"]);
    assert_eq!(created.code, 0, "{}", created.stderr);
    for file in [
        "architecture.toml",
        "lean-inventory.json",
        "BUILD.bazel",
        "README.md",
        "spec/lakefile.lean",
        "spec/lean-toolchain",
        "spec/PumpStation.lean",
        "spec/PumpStation/Monitor.lean",
        "spec/PumpStation/Wiring.lean",
        "verification/lakefile.lean",
        "verification/lean-toolchain",
        "verification/README.md",
    ] {
        assert!(dir.join(file).is_file(), "{file} missing");
    }
    // The verification package depends on the project's own specification.
    let lakefile = fs::read_to_string(dir.join("verification/lakefile.lean")).unwrap();
    assert!(lakefile.contains("require «pump_station_spec» from \"..\" / \"spec\""));
    // Without a workspace flag, no workspace files are written.
    assert!(!dir.join("MODULE.bazel").exists());
    // The names are the project's own, everywhere.
    let spec = fs::read_to_string(dir.join("spec/PumpStation/Monitor.lean")).unwrap();
    assert!(spec.contains("namespace PumpStation.Monitor"));
    assert!(!spec.contains("Starter"));
    let model = dir.join("architecture.toml").display().to_string();
    let validated = l2r(&["validate", &model]);
    assert_eq!(validated.code, 0, "{}", validated.stderr);
    assert!(validated.stdout.contains("model `pump_station` is valid"));

    // The project can be taken through the whole workflow as it stands.
    let digest = digest_from(&l2r(&["plan", &model]).stdout);
    assert_eq!(approve(&model, &digest).code, 0);
    assert_eq!(l2r(&["generate", &model]).code, 0);
    assert_eq!(l2r(&["check", &model]).code, 0);

    // A second init into the same directory writes nothing.
    let before = fs::read_to_string(dir.join("architecture.toml")).unwrap();
    let again = l2r(&["init", &target, "--id", "other"]);
    assert_eq!(again.code, 1);
    assert!(again.stderr.contains("already exists"), "{}", again.stderr);
    assert_eq!(
        fs::read_to_string(dir.join("architecture.toml")).unwrap(),
        before
    );
}

#[test]
fn init_with_a_workspace_pins_l2r() {
    let dir = scratch_dir("init-ws").join("pump");
    let target = dir.display().to_string();
    let created = l2r(&[
        "init",
        &target,
        "--id",
        "pump_station",
        "--lean-root",
        "Pump",
        "--workspace-commit",
        "0123abc",
    ]);
    assert_eq!(created.code, 0, "{}", created.stderr);
    let module = fs::read_to_string(dir.join("MODULE.bazel")).unwrap();
    assert!(module.contains("module(name = \"pump_station\""));
    assert!(module.contains("commit = \"0123abc\""));
    assert!(dir.join(".bazelversion").is_file());
    assert!(dir.join("spec/Pump/Monitor.lean").is_file());
    // The Lean library is pinned to the same commit as the Bazel module.
    let lakefile = fs::read_to_string(dir.join("verification/lakefile.lean")).unwrap();
    assert!(lakefile.contains("@ \"0123abc\" / \"lean\""), "{lakefile}");
}

#[test]
fn init_rejects_unusable_names() {
    let dir = scratch_dir("init-bad").join("p");
    let target = dir.display().to_string();
    for args in [
        vec!["init", target.as_str(), "--id", "Pump"],
        vec![
            "init",
            target.as_str(),
            "--id",
            "pump",
            "--lean-root",
            "pump",
        ],
        vec!["init", target.as_str()],
    ] {
        let run = l2r(&args);
        assert_ne!(run.code, 0, "{args:?}");
        assert!(!dir.exists(), "{args:?} wrote something");
    }
}

#[test]
fn the_work_packet_names_what_may_be_edited_and_what_must_hold() {
    let (_dir, model) = project("packet");
    let packet = l2r(&["packet", &model, "signal"]);
    assert_eq!(packet.code, 0, "{}", packet.stderr);
    let text = packet.stdout;
    assert!(text.contains("# Work packet: Crossing signal (`signal`)"));
    // Only the implementation file is editable; the contract is read-only.
    let editable = text
        .split("## Files you may edit")
        .nth(1)
        .and_then(|rest| rest.split("## Contract").next())
        .unwrap();
    assert!(editable.contains("scaffold/src/crossing/signal/imp.rs"));
    assert!(!editable.contains("mod.rs"));
    assert!(text.contains("`Crossing.Signal.step`"));
    assert!(text.contains("`Crossing.Signal.no_conflicting_release`"));
    assert!(text.contains("edge `latch_to_signal`"));

    // A composite's packet lists the translations its routes file implements.
    let composite = l2r(&["packet", &model, "crossing"]).stdout;
    assert!(composite.contains("| `latch_to_signal` | `Crossing.Routes.latchToSignal` |"));

    let unknown = l2r(&["packet", &model, "nobody"]);
    assert_eq!(unknown.code, 1);
    assert!(unknown.stderr.contains("no component `nobody`"));
}

#[test]
fn inventory_needs_a_lake_project_and_root_modules() {
    let dir = scratch_dir("inventory");
    let project = dir.display().to_string();
    let out = dir.join("out.json").display().to_string();
    let no_project = l2r(&["inventory", &project, "--out", &out, "Root"]);
    assert_eq!(no_project.code, 1);
    assert!(no_project.stderr.contains("not a Lake project"));
    assert!(!dir.join("out.json").exists());
    assert_ne!(l2r(&["inventory", &project, "--out", &out]).code, 0);
}

#[test]
fn conformance_prints_both_sides_for_a_hierarchical_model_only() {
    let dir = scratch_dir("conformance").join("p");
    let target = dir.display().to_string();
    assert_eq!(l2r(&["init", &target, "--id", "pump_station"]).code, 0);
    let model = dir.join("architecture.toml").display().to_string();
    let rust = l2r(&["conformance", &model, "--rust"]);
    assert_eq!(rust.code, 0, "{}", rust.stderr);
    assert!(
        rust.stdout
            .contains("use pump_station_scaffold::pump_station::{Progress, System};")
    );
    let lean = l2r(&["conformance", &model, "--lean"]);
    assert_eq!(lean.code, 0, "{}", lean.stderr);
    assert!(lean.stdout.contains("import L2RDerived.pump_station"));
    assert_eq!(l2r(&["conformance", &model]).code, 2);

    // The older profiles have their own, hand-written conformance lanes.
    let (_dir, v0) = project("conformance-v0");
    let refused = l2r(&["conformance", &v0, "--rust"]);
    assert_eq!(refused.code, 1);
    assert!(
        refused.stderr.contains("v2 models only"),
        "{}",
        refused.stderr
    );
}

#[test]
fn approve_replaces_a_link_planted_at_the_record_and_leaves_its_target_alone() {
    let (dir, model) = project("approve-link");
    let victim = dir.join("victim.txt");
    fs::write(&victim, "victim\n").unwrap();
    let record = dir.join("architecture.approval.json");
    std::os::unix::fs::symlink(&victim, &record).unwrap();

    let digest = digest_from(&l2r(&["plan", &model]).stdout);
    let approved = approve(&model, &digest);
    assert_eq!(approved.code, 0, "{}", approved.stderr);

    assert_eq!(fs::read_to_string(&victim).unwrap(), "victim\n");
    let kind = fs::symlink_metadata(&record).unwrap().file_type();
    assert!(kind.is_file(), "the record is a regular file, not the link");
    assert!(fs::read_to_string(&record).unwrap().contains(&digest));
}
