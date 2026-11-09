//! Test-only: runs `l2r generate` for a model but aborts the process after a
//! given number of committed steps, leaving a real interrupted generation.
//!
//! Usage: `l2r_crash_after <model.toml> <steps>`

use std::path::Path;

use l2r_apply::fault::{self, Mode};
use l2r_model::load;
use l2r_plan::{GeneratorIdentity, plan};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model_path = Path::new(&args[1]);
    let steps: usize = args[2].parse().expect("step count");
    let base = model_path.parent().expect("model has a directory");
    let model = load::load_model(model_path).expect("model loads");
    let inventory =
        load::load_inventory(&base.join(&model.system.lean_inventory)).expect("inventory loads");
    let validated = l2r_validate::validate(&model, &inventory).expect("model is valid");
    let plan = plan(&validated, &GeneratorIdentity::current()).expect("plans");
    let stem = model_path.file_stem().expect("stem").to_string_lossy();
    let record =
        std::fs::read_to_string(base.join(format!("{stem}.approval.json"))).expect("approval");
    let approval = l2r_apply::approval::from_json(&record).expect("approval parses");
    fault::arm(steps, Mode::Abort);
    let result = l2r_apply::apply(&plan, &approval, &base.join(&model.system.output_dir));
    eprintln!("not aborted: {result:?}");
    std::process::exit(3);
}
