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

/// The hierarchical (`v2`) example model: the tank filling station.
pub const TANK_MODEL: &str = include_str!("../../../examples/tank_station/architecture.toml");

/// The tank station's Lean inventory (`l2r.lean-inventory/v1`).
pub const TANK_INVENTORY: &str = include_str!("../../../examples/tank_station/lean-inventory.json");

/// Parses the tank station model.
pub fn tank_model() -> Model {
    load::parse_model(TANK_MODEL).expect("tank model parses")
}

/// Parses the tank station inventory.
pub fn tank_inventory() -> Inventory {
    load::parse_inventory(TANK_INVENTORY).expect("tank inventory parses")
}

/// The relay generator fixture (`v2`): deep nesting, port chains, two routes
/// to one receiver.
pub const RELAY_MODEL: &str = include_str!("../../../examples/relay/architecture.toml");

/// The relay fixture's Lean inventory.
pub const RELAY_INVENTORY: &str = include_str!("../../../examples/relay/lean-inventory.json");

/// Parses the relay model.
pub fn relay_model() -> Model {
    load::parse_model(RELAY_MODEL).expect("relay model parses")
}

/// Parses the relay inventory.
pub fn relay_inventory() -> Inventory {
    load::parse_inventory(RELAY_INVENTORY).expect("relay inventory parses")
}

/// The starter project `l2r init` is based on (`v2`).
pub const STARTER_MODEL: &str = include_str!("../../../examples/starter/architecture.toml");

/// The starter project's Lean inventory.
pub const STARTER_INVENTORY: &str = include_str!("../../../examples/starter/lean-inventory.json");

/// Parses the starter model.
pub fn starter_model() -> Model {
    load::parse_model(STARTER_MODEL).expect("starter model parses")
}

/// Parses the starter inventory.
pub fn starter_inventory() -> Inventory {
    load::parse_inventory(STARTER_INVENTORY).expect("starter inventory parses")
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
