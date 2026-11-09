//! Shared fixtures for L2R tests: the crossing example and scratch directories.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use l2r_model::{Inventory, Model, load};

/// The example architecture model, as committed.
pub const EXAMPLE_MODEL: &str = include_str!("../../../examples/crossing/architecture.toml");

/// The example Lean inventory, as committed.
pub const EXAMPLE_INVENTORY: &str = include_str!("../../../examples/crossing/lean-inventory.json");

/// Parses the example model.
pub fn example_model() -> Model {
    load::parse_model(EXAMPLE_MODEL).expect("example model parses")
}

/// Parses the example inventory.
pub fn example_inventory() -> Inventory {
    load::parse_inventory(EXAMPLE_INVENTORY).expect("example inventory parses")
}

/// The composite/v1 example model, as committed.
pub const EXAMPLE_V1_MODEL: &str = include_str!("../../../examples/crossing_v1/architecture.toml");

/// The composite/v1 example Lean inventory, as committed.
pub const EXAMPLE_V1_INVENTORY: &str =
    include_str!("../../../examples/crossing_v1/lean-inventory.json");

/// Parses the composite/v1 example model.
pub fn example_v1_model() -> Model {
    load::parse_model(EXAMPLE_V1_MODEL).expect("v1 example model parses")
}

/// Parses the composite/v1 example inventory.
pub fn example_v1_inventory() -> Inventory {
    load::parse_inventory(EXAMPLE_V1_INVENTORY).expect("v1 example inventory parses")
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// Creates a fresh, empty directory under the test's scratch area.
pub fn scratch_dir(label: &str) -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = base.join(format!("l2r-{label}-{}-{n}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear scratch dir");
    }
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}
