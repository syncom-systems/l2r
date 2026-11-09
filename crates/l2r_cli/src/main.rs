//! `l2r`: validate an architecture model, preview and approve its generation
//! plan, and write the approved Rust scaffold.

mod commands;
mod diff;
mod project;
mod report;
mod session;

use std::path::PathBuf;
use std::process::ExitCode;

use commands::Outcome;

const USAGE: &str = "\
L2R — human-approved architecture and deterministic Rust scaffolding

Usage:
  l2r init <dir> --id <id> [--lean-root <Name>]     Create a new project: model, Lean spec, inventory
              [--workspace-commit <sha> | --workspace-path <l2r checkout>]
                                                    ... and, with either, a new Bazel workspace around it
  l2r inventory <lake-dir> --out <file> <Root>...   Build the Lean project and export its inventory (runs `lake`)
  l2r validate <model.toml>                         Validate the model against its Lean inventory
  l2r plan     <model.toml> [--json]                Show the deterministic plan and its digest
  l2r preview  <model.toml> [--summary]             Show the plan, its effect on disk, and every change
  l2r approve  <model.toml> --digest <d> --by <n>   Record approval of the plan with digest <d>
  l2r generate <model.toml>                         Write the approved plan (refuses stale approvals)
  l2r recover  <model.toml>                         Finish an interrupted generation of the approved plan
  l2r check    <model.toml>                         Check the output for drift, staleness and budgets
  l2r render   <model.toml>                         Print a Mermaid view of the architecture
  l2r packet   <model.toml> <component>             Print the work packet for implementing one component
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
    match command.as_str() {
        "init" => return init(rest),
        "inventory" => return inventory(rest),
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
        "packet" => match rest {
            [component] => Ok(project::packet(&model, component)),
            _ => Err(usage_error(
                "`packet` needs a model file and one component id",
            )),
        },
        "lean-composition" => only_flags(rest, &[]).map(|_| commands::lean_composition(&model)),
        other => Err(usage_error(&format!("unknown command `{other}`"))),
    };
    result.unwrap_or_else(|outcome| outcome)
}

/// Flag values (`(flag, value)`, the value empty for a switch) and positional arguments.
type Parsed<'a> = (Vec<(&'a str, &'a str)>, Vec<&'a str>);

/// Splits `rest` into flag values and positional arguments. Every flag in
/// `flags` takes one value; `switches` take none.
fn parse_flags<'a>(
    rest: &'a [String],
    flags: &[&str],
    switches: &[&str],
) -> Result<Parsed<'a>, Outcome> {
    let mut values = Vec::new();
    let mut positional = Vec::new();
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        if flags.contains(&arg.as_str()) {
            let Some(value) = iter.next() else {
                return Err(usage_error(&format!("`{arg}` needs a value")));
            };
            values.push((arg.as_str(), value.as_str()));
        } else if switches.contains(&arg.as_str()) {
            values.push((arg.as_str(), ""));
        } else if arg.starts_with("--") {
            return Err(usage_error(&format!("unexpected argument `{arg}`")));
        } else {
            positional.push(arg.as_str());
        }
    }
    Ok((values, positional))
}

fn init(rest: &[String]) -> Outcome {
    let flags = [
        "--id",
        "--lean-root",
        "--workspace-commit",
        "--workspace-path",
    ];
    let (values, positional) = match parse_flags(rest, &flags, &[]) {
        Ok(parsed) => parsed,
        Err(outcome) => return outcome,
    };
    let value = |flag: &str| values.iter().find(|(f, _)| *f == flag).map(|(_, v)| *v);
    let ([dir], Some(id)) = (positional.as_slice(), value("--id")) else {
        return usage_error("`init` needs a directory and --id <system id>");
    };
    let lean_root = match value("--lean-root") {
        Some(root) => root.to_string(),
        None => project::default_lean_root(id).unwrap_or_default(),
    };
    let workspace = match (value("--workspace-commit"), value("--workspace-path")) {
        (Some(_), Some(_)) => {
            return usage_error("use one of --workspace-commit and --workspace-path");
        }
        (Some(commit), None) => Some(project::L2rSource::Commit(commit.to_string())),
        (None, Some(path)) => Some(project::L2rSource::Path(path.to_string())),
        (None, None) => None,
    };
    project::init(&project::InitOptions {
        dir: PathBuf::from(dir),
        id: id.to_string(),
        lean_root,
        workspace,
    })
}

fn inventory(rest: &[String]) -> Outcome {
    let (values, positional) = match parse_flags(rest, &["--out"], &["--v0"]) {
        Ok(parsed) => parsed,
        Err(outcome) => return outcome,
    };
    let out = values.iter().find(|(f, _)| *f == "--out").map(|(_, v)| *v);
    let (Some((project, roots)), Some(out)) = (positional.split_first(), out) else {
        return usage_error(
            "`inventory` needs a Lake project directory, --out <file> and root modules",
        );
    };
    let roots: Vec<String> = roots.iter().map(|r| r.to_string()).collect();
    let v0 = values.iter().any(|(f, _)| *f == "--v0");
    project::inventory(
        std::path::Path::new(project),
        std::path::Path::new(out),
        v0,
        &roots,
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = run(&args);
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    ExitCode::from(u8::try_from(outcome.code).unwrap_or(1))
}
