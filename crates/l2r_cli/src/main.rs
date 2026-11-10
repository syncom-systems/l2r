//! `l2r`: validate an architecture model, preview and approve its generation
//! plan, and write the approved Rust scaffold.

mod commands;
mod diff;
mod report;
mod session;

use std::path::PathBuf;
use std::process::ExitCode;

use commands::Outcome;

const USAGE: &str = "\
L2R — human-approved architecture and deterministic Rust scaffolding

Usage:
  l2r validate <model.toml>                         Validate the model against its Lean inventory
  l2r plan     <model.toml> [--json]                Show the deterministic plan and its digest
  l2r preview  <model.toml> [--summary]             Show the plan, its effect on disk, and every change
  l2r approve  <model.toml> --digest <d> --by <n>   Record approval of the plan with digest <d>
  l2r generate <model.toml>                         Write the approved plan (refuses stale approvals)
  l2r recover  <model.toml>                         Finish an interrupted generation of the approved plan
  l2r check    <model.toml>                         Check the output for drift, staleness and budgets
  l2r render   <model.toml>                         Print a Mermaid view of the architecture
  l2r lean-composition <model.toml>                 Print the derived Lean network of the composition
  l2r --version
";

fn usage_error(message: &str) -> Outcome {
    Outcome {
        stdout: String::new(),
        stderr: format!("{message}\n\n{USAGE}"),
        code: 2,
    }
}

/// Returns the value following `flag`, if present.
fn flag_value<'a>(rest: &'a [String], flag: &str) -> Option<&'a str> {
    rest.iter()
        .position(|a| a == flag)
        .and_then(|i| rest.get(i + 1))
        .map(String::as_str)
}

fn only_flags(rest: &[String], allowed: &[&str]) -> Result<(), Outcome> {
    match rest.iter().find(|a| !allowed.contains(&a.as_str())) {
        Some(unknown) => Err(usage_error(&format!("unexpected argument `{unknown}`"))),
        None => Ok(()),
    }
}

fn run(args: &[String]) -> Outcome {
    let Some((command, rest)) = args.split_first() else {
        return Outcome {
            stdout: USAGE.into(),
            stderr: String::new(),
            code: 0,
        };
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            return Outcome {
                stdout: USAGE.into(),
                stderr: String::new(),
                code: 0,
            };
        }
        "--version" | "-V" => {
            return Outcome {
                stdout: format!("l2r {}\n", l2r_plan::GENERATOR_VERSION),
                stderr: String::new(),
                code: 0,
            };
        }
        _ => {}
    }
    let Some((model, rest)) = rest.split_first() else {
        return usage_error(&format!("`{command}` needs a model file"));
    };
    let model = PathBuf::from(model);
    let result = match command.as_str() {
        "validate" => only_flags(rest, &[]).map(|_| commands::validate(&model)),
        "plan" => only_flags(rest, &["--json"])
            .map(|_| commands::plan(&model, rest.iter().any(|a| a == "--json"))),
        "preview" => only_flags(rest, &["--summary"])
            .map(|_| commands::preview(&model, !rest.iter().any(|a| a == "--summary"))),
        "approve" => match (flag_value(rest, "--digest"), flag_value(rest, "--by")) {
            (Some(digest), Some(by)) if rest.len() == 4 => {
                Ok(commands::approve_plan(&model, digest, by))
            }
            _ => Err(usage_error(
                "`approve` needs --digest <digest> and --by <name>",
            )),
        },
        "generate" => only_flags(rest, &[]).map(|_| commands::generate(&model)),
        "recover" => only_flags(rest, &[]).map(|_| commands::recover_generation(&model)),
        "check" => only_flags(rest, &[]).map(|_| commands::check_output(&model)),
        "render" => only_flags(rest, &[]).map(|_| commands::render(&model)),
        "lean-composition" => only_flags(rest, &[]).map(|_| commands::lean_composition(&model)),
        other => Err(usage_error(&format!("unknown command `{other}`"))),
    };
    result.unwrap_or_else(|outcome| outcome)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = run(&args);
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    ExitCode::from(u8::try_from(outcome.code).unwrap_or(1))
}
