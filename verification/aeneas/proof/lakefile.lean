import Lake
open Lake DSL

/-!
Refinement proof: the Aeneas translation of the crossing signal's Rust
implementation against the Lean specification.

The Aeneas Lean library is not vendored. Point Lake at a built checkout of the
pinned Aeneas release with `-K aeneas=<path-to-aeneas>/backends/lean` (the
`run.sh` script does this from `AENEAS_DIR`).
-/

package crossing_refinement

require aeneas from (get_config? aeneas |>.getD "aeneas-not-configured")
require crossing_spec from "../../../examples/crossing/spec"

lean_lib Extracted

@[default_target]
lean_lib Refinement
