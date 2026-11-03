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
