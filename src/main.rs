//! L2R CLI bootstrap. Architecture validation and generation are NOT implemented yet.

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() -> &'static str {
    "L2R - Lean-first architecture, human-approved Rust scaffolding\n\n\
Usage:\n\
  l2r --help        Show this help\n\
  l2r --version     Show package version\n\n\
Planned commands (not implemented):\n\
  validate           Validate an architectural model and Lean references\n\
  plan               Show a deterministic file-generation plan\n\
  render             Generate architecture views\n\
  generate           Generate approved Rust scaffolding\n\
  check              Check drift, plan provenance, and implementation boundaries\n"
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => print!("{}", usage()),
        [arg] if arg == "--help" || arg == "-h" || arg == "help" => print!("{}", usage()),
        [arg] if arg == "--version" || arg == "-V" => println!("l2r {VERSION}"),
        _ => {
            eprintln!("Unimplemented or unknown command.\n\n{}", usage());
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_identifies_unimplemented_commands() {
        assert!(usage().contains("not implemented"));
        assert!(usage().contains("generate"));
    }
}
