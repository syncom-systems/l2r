#!/usr/bin/env bash
# Regenerates a Lean declaration inventory for L2R.
#
#   tools/lean_inventory/export.sh <lake-project-dir> <output.json> <RootModule>...
#
# Builds the Lake project with the toolchain its `lean-toolchain` file pins, then runs
# the exporter against the compiled modules. Lean is not yet a hermetic Bazel
# toolchain in this repository, so this step runs outside Bazel; the inventory it
# writes is committed, and its digest is bound into every generation plan.
set -euo pipefail
project="$1"; out="$2"; shift 2
here="$(cd "$(dirname "$0")" && pwd)"
out_abs="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
cd "$project"
lake build >&2
lake env lean --run "$here/Main.lean" "$@" > "$out_abs.tmp"
mv "$out_abs.tmp" "$out_abs"
echo "wrote $out_abs" >&2
