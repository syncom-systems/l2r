//! `l2r`: validate an architecture model, preview and approve its generation
//! plan, and write the approved Rust scaffold.

mod commands;
mod diff;
mod init;
mod inventory;
mod packet;
mod report;
mod session;

use std::path::{Path, PathBuf};
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
  l2r conformance <model.toml> --rust | --lean      Print one side of the derived conformance harness (schema v2)
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
    dispatch(args).unwrap_or_else(|usage_error| usage_error)
}

/// Runs the command that `args` name. The error is a usage error.
fn dispatch(args: &[String]) -> Result<Outcome, Outcome> {
    let Some((command, rest)) = args.split_first() else {
        return Ok(Outcome::ok(USAGE.into()));
    };
    match command.as_str() {
        "--help" | "-h" | "help" => Ok(Outcome::ok(USAGE.into())),
        "--version" | "-V" => Ok(Outcome::ok(format!(
            "l2r {}\n",
            l2r_plan::GENERATOR_VERSION
        ))),
        "init" => init(rest),
        "inventory" => inventory(rest),
        command => model_command(command, rest),
    }
}

/// The commands on a model file, which is their first argument.
fn model_command(command: &str, rest: &[String]) -> Result<Outcome, Outcome> {
    let Some((model, rest)) = rest.split_first() else {
        return Err(usage_error(&format!("`{command}` needs a model file")));
    };
    let model = Path::new(model);
    match command {
        "plan" => only_flags(rest, &["--json"])
            .map(|()| commands::plan(model, rest.iter().any(|a| a == "--json"))),
        "preview" => only_flags(rest, &["--summary"])
            .map(|()| commands::preview(model, !rest.iter().any(|a| a == "--summary"))),
        "approve" => approve(model, rest),
        "conformance" => conformance(model, rest),
        "packet" => packet(model, rest),
        command => model_only_command(command, model, rest),
    }
}

/// The commands whose only argument is the model file.
fn model_only_command(command: &str, model: &Path, rest: &[String]) -> Result<Outcome, Outcome> {
    let run: fn(&Path) -> Outcome = match command {
        "validate" => commands::validate,
        "generate" => commands::generate,
        "recover" => commands::recover_generation,
        "check" => commands::check_output,
        "render" => commands::render,
        "lean-composition" => commands::lean_composition,
        other => return Err(usage_error(&format!("unknown command `{other}`"))),
    };
    only_flags(rest, &[])?;
    Ok(run(model))
}

fn approve(model: &Path, rest: &[String]) -> Result<Outcome, Outcome> {
    match (flag_value(rest, "--digest"), flag_value(rest, "--by")) {
        (Some(digest), Some(by)) if rest.len() == 4 => {
            Ok(commands::approve_plan(model, digest, by))
        }
        _ => Err(usage_error(
            "`approve` needs --digest <digest> and --by <name>",
        )),
    }
}

fn conformance(model: &Path, rest: &[String]) -> Result<Outcome, Outcome> {
    let side = match rest {
        [side] if side == "--rust" => l2r_plan::ConformanceSide::Rust,
        [side] if side == "--lean" => l2r_plan::ConformanceSide::Lean,
        _ => return Err(usage_error("`conformance` needs --rust or --lean")),
    };
    Ok(commands::conformance(model, side))
}

fn packet(model: &Path, rest: &[String]) -> Result<Outcome, Outcome> {
    match rest {
        [component] => Ok(packet::packet(model, component)),
        _ => Err(usage_error(
            "`packet` needs a model file and one component id",
        )),
    }
}

/// The arguments of a command that takes flags with values.
struct Arguments<'a> {
    /// `(flag, value)` in the order given; the value is empty for a switch.
    flags: Vec<(&'a str, &'a str)>,
    positional: Vec<&'a str>,
}

impl<'a> Arguments<'a> {
    /// Splits `rest` into flag values and positional arguments. Every flag in
    /// `flags` takes one value; `switches` take none.
    fn parse(rest: &'a [String], flags: &[&str], switches: &[&str]) -> Result<Self, Outcome> {
        let mut arguments = Arguments {
            flags: Vec::new(),
            positional: Vec::new(),
        };
        let mut iter = rest.iter().map(String::as_str);
        while let Some(arg) = iter.next() {
            if flags.contains(&arg) {
                let value = iter
                    .next()
                    .ok_or_else(|| usage_error(&format!("`{arg}` needs a value")))?;
                arguments.flags.push((arg, value));
            } else if switches.contains(&arg) {
                arguments.flags.push((arg, ""));
            } else if arg.starts_with("--") {
                return Err(usage_error(&format!("unexpected argument `{arg}`")));
            } else {
                arguments.positional.push(arg);
            }
        }
        Ok(arguments)
    }

    /// The value of `flag` where it is first given.
    fn value(&self, flag: &str) -> Option<&'a str> {
        self.flags.iter().find(|(f, _)| *f == flag).map(|(_, v)| *v)
    }

    /// True when the switch is given.
    fn has(&self, switch: &str) -> bool {
        self.flags.iter().any(|(f, _)| *f == switch)
    }
}

/// How the workspace that `init` is asked to write refers to L2R, if it is
/// asked to write one.
fn workspace_source(arguments: &Arguments) -> Result<Option<init::L2rSource>, Outcome> {
    let commit = arguments.value("--workspace-commit");
    let path = arguments.value("--workspace-path");
    match (commit, path) {
        (Some(_), Some(_)) => Err(usage_error(
            "use one of --workspace-commit and --workspace-path",
        )),
        (Some(commit), None) => Ok(Some(init::L2rSource::Commit(commit.to_string()))),
        (None, Some(path)) => Ok(Some(init::L2rSource::Path(path.to_string()))),
        (None, None) => Ok(None),
    }
}

fn init(rest: &[String]) -> Result<Outcome, Outcome> {
    let flags = [
        "--id",
        "--lean-root",
        "--workspace-commit",
        "--workspace-path",
    ];
    let arguments = Arguments::parse(rest, &flags, &[])?;
    let ([dir], Some(id)) = (arguments.positional.as_slice(), arguments.value("--id")) else {
        return Err(usage_error("`init` needs a directory and --id <system id>"));
    };
    let lean_root = match arguments.value("--lean-root") {
        Some(root) => root.to_string(),
        None => init::default_lean_root(id).unwrap_or_default(),
    };
    let workspace = workspace_source(&arguments)?;
    Ok(init::init(&init::NewProject {
        dir: PathBuf::from(dir),
        id: id.to_string(),
        lean_root,
        workspace,
    }))
}

fn inventory(rest: &[String]) -> Result<Outcome, Outcome> {
    let arguments = Arguments::parse(rest, &["--out"], &["--v0"])?;
    let (Some((project, roots)), Some(out)) =
        (arguments.positional.split_first(), arguments.value("--out"))
    else {
        return Err(usage_error(
            "`inventory` needs a Lake project directory, --out <file> and root modules",
        ));
    };
    let roots: Vec<String> = roots.iter().map(|r| r.to_string()).collect();
    Ok(inventory::inventory(
        Path::new(project),
        Path::new(out),
        arguments.has("--v0"),
        &roots,
    ))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = run(&args);
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    ExitCode::from(u8::try_from(outcome.code).unwrap_or(1))
}
