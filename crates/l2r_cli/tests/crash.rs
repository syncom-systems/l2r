//! A generation killed part-way (SIGABRT) is finished by the real `l2r recover`.

use std::fs;
use std::process::Command;

use l2r_testing::{EXAMPLE_INVENTORY, EXAMPLE_MODEL, scratch_dir};

fn run(bin_var: &str, args: &[&str]) -> std::process::Output {
    let bin = std::env::var(bin_var).expect("binary path set by Bazel");
    Command::new(bin).args(args).output().expect("runs")
}

fn l2r(args: &[&str]) -> (i32, String) {
    let out = run("L2R_BIN", args);
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.code().unwrap_or(-1), text)
}

fn approve_current(model: &str) {
    let (_, plan) = l2r(&["plan", model]);
    let digest = plan
        .lines()
        .find_map(|l| l.strip_prefix("Plan digest: "))
        .unwrap()
        .to_string();
    assert_eq!(
        l2r(&["approve", model, "--digest", &digest, "--by", "crash test"]).0,
        0
    );
}

#[test]
fn an_aborted_generation_is_completed_by_recover() {
    for steps in [0, 1, 2] {
        let dir = scratch_dir(&format!("crash-{steps}"));
        fs::write(dir.join("architecture.toml"), EXAMPLE_MODEL).unwrap();
        fs::write(dir.join("lean-inventory.json"), EXAMPLE_INVENTORY).unwrap();
        let model = dir.join("architecture.toml").display().to_string();
        approve_current(&model);
        assert_eq!(l2r(&["generate", &model]).0, 0);
        fs::write(
            dir.join("scaffold/src/crossing/signal/imp.rs"),
            "// work in progress\n",
        )
        .unwrap();

        let edited =
            EXAMPLE_MODEL.replace("Owns no behavioral state of its own.", "Owns no state.");
        let edited = edited.replace("acknowledges the request.", "acknowledges it.");
        fs::write(dir.join("architecture.toml"), edited).unwrap();
        approve_current(&model);

        let crashed = run("CRASH_BIN", &[&model, &steps.to_string()]);
        assert!(
            crashed.status.code().is_none(),
            "steps={steps}: expected death by signal, got {:?}",
            crashed.status
        );
        assert!(dir.join("scaffold/l2r.journal.json").exists());

        let (code, text) = l2r(&["generate", &model]);
        assert_eq!(code, 1);
        assert!(text.contains("l2r recover"), "{text}");

        let (code, text) = l2r(&["recover", &model]);
        assert_eq!(code, 0, "steps={steps}: {text}");
        let (code, text) = l2r(&["check", &model]);
        assert_eq!(code, 0, "steps={steps}: {text}");
        assert_eq!(
            fs::read_to_string(dir.join("scaffold/src/crossing/signal/imp.rs")).unwrap(),
            "// work in progress\n"
        );
    }
}
