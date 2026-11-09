#!/usr/bin/env bash
# Rust-to-Lean refinement experiment for the crossing signal.
#
#   AENEAS_DIR=<built Aeneas checkout> verification/aeneas/run.sh
#
# Requires: AENEAS_DIR containing bin/aeneas, charon/bin/charon and a built
# backends/lean; the Rust nightly Charon was built against (RUSTUP_HOME may point
# at an isolated rustup); elan with Lean v4.31.0. Not part of `bazel test //...`:
# these tools are not yet hermetic Bazel toolchains (see README.md here).
#
# Steps, each recorded under evidence/ with local paths replaced by placeholders:
#   0. inventory the committed Lean inventory must equal a fresh export of the spec;
#   1. extract   the committed signal module with Charon + Aeneas, and require the
#                committed translation (proof/Extracted) to be byte-identical;
#   2. prove     build the refinement proof (must succeed, no sorry);
#   3. defect    re-extract with an injected defect and require the proof to FAIL.
set -euo pipefail
: "${AENEAS_DIR:?set AENEAS_DIR to a built Aeneas checkout}"
repo="$(cd "$(dirname "$0")/../.." && pwd)"
here="$repo/verification/aeneas"
evidence="$here/evidence"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$evidence"

sanitize() { sed -e "s#$work#<tmp>#g" -e "s#$repo#<repo>#g" -e "s#$AENEAS_DIR#<aeneas>#g" -e "s#$HOME#<home>#g"; }
record() { local name="$1"; shift; { echo "\$ $*"; "$@" 2>&1; echo "exit status: $?"; } | sanitize > "$evidence/$name.log" || true; }

extract() { # <crate-root.rs> <out-dir>
  "$AENEAS_DIR/charon/bin/charon" rustc --preset=aeneas --dest-file "$2/signal_crate.llbc" -- "$1" --crate-type=lib --edition=2024
  "$AENEAS_DIR/bin/aeneas" -backend lean "$2/signal_crate.llbc" -dest "$2"
}

cd "$repo"
{
  echo "charon:  $("$AENEAS_DIR/charon/bin/charon" version 2>&1)"
  echo "aeneas:  $(git -C "$AENEAS_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "charon pin (from aeneas): $(grep -v '^#' "$AENEAS_DIR/charon-pin" 2>/dev/null || echo unknown)"
  echo "lean:    $(cat "$here/proof/lean-toolchain")"
} | sanitize > "$evidence/toolchain.txt"

# 0. The committed Lean inventory must match the Lean sources it claims to describe.
"$repo/tools/lean_inventory/export.sh" "$repo/examples/crossing/spec" "$work/inventory.json" Crossing 2> /dev/null
if cmp -s "$work/inventory.json" "$repo/examples/crossing/lean-inventory.json"; then
  echo "committed Lean inventory is current" > "$evidence/inventory-freshness.txt"
else
  diff "$repo/examples/crossing/lean-inventory.json" "$work/inventory.json" | sanitize > "$evidence/inventory-freshness.txt" || true
  echo "FAIL: examples/crossing/lean-inventory.json is stale; re-export it" >&2
  exit 1
fi

# 1. Extract the committed code (relative crate root keeps provenance paths stable).
mkdir -p "$work/fresh"
record extract extract verification/aeneas/signal_crate.rs "$work/fresh"
if cmp -s "$work/fresh/SignalCrate.lean" "$here/proof/Extracted/SignalCrate.lean"; then
  echo "committed translation is current" > "$evidence/freshness.txt"
else
  diff "$here/proof/Extracted/SignalCrate.lean" "$work/fresh/SignalCrate.lean" | sanitize > "$evidence/freshness.txt" || true
  echo "FAIL: proof/Extracted/SignalCrate.lean is stale; see evidence/freshness.txt" >&2
  exit 1
fi

# 2. Prove.
cd "$here/proof"
record prove lake -K "aeneas=$AENEAS_DIR/backends/lean" build
grep -q "exit status: 0" "$evidence/prove.log" || { echo "FAIL: refinement proof did not build" >&2; exit 1; }
if grep -q "sorryAx" "$evidence/prove.log"; then echo "FAIL: proof depends on sorry" >&2; exit 1; fi

# 3. Inject a defect: a crossing request jumps straight to walk, skipping amber.
mkdir -p "$work/defect/src/signal" "$work/defect/out"
cp "$repo/examples/crossing/scaffold/src/crossing/signal/"{mod.rs,imp.rs} "$work/defect/src/signal/"
sed -i.bak 's/(State::VehiclesGreen, Input::CrossingRequested) => (State::VehiclesAmber, None)/(State::VehiclesGreen, Input::CrossingRequested) => (State::Walk, None)/' "$work/defect/src/signal/imp.rs"
if cmp -s "$work/defect/src/signal/imp.rs" "$work/defect/src/signal/imp.rs.bak"; then echo "FAIL: defect was not injected" >&2; exit 1; fi
# Same crate name as the real harness, so the translation lands in the same namespace.
printf '#[path = "%s"]\npub mod signal;\n' "$work/defect/src/signal/mod.rs" > "$work/defect/src/signal_crate.rs"
extract "$work/defect/src/signal_crate.rs" "$work/defect/out" > /dev/null 2>&1
{
  echo "import Aeneas"
  echo "import Crossing.Signal"
  grep -v '^import ' "$work/defect/out/SignalCrate.lean"
  grep -v '^import ' "$here/proof/Refinement.lean"
} > "$work/defect/DefectRefinement.lean"
record defect lake -K "aeneas=$AENEAS_DIR/backends/lean" env lean "$work/defect/DefectRefinement.lean"
if grep -q "exit status: 0" "$evidence/defect.log"; then echo "FAIL: the injected defect was not detected" >&2; exit 1; fi
# The rejection must come from the refinement theorem, not from a broken setup.
step_line=$(grep -n "^theorem step_refines" "$work/defect/DefectRefinement.lean" | cut -d: -f1)
errors=$(grep -E "DefectRefinement.lean:[0-9]+:[0-9]+: error" "$evidence/defect.log" | sed -E 's/.*DefectRefinement.lean:([0-9]+):.*/\1/' | sort -u)
for line in $errors; do
  if [ "$line" -lt "$step_line" ] || [ "$line" -gt $((step_line + 3)) ]; then
    echo "FAIL: unexpected error outside step_refines (line $line); the defect run is not a valid detection" >&2; exit 1
  fi
done
[ -n "$errors" ] || { echo "FAIL: no error recorded for the defect" >&2; exit 1; }

echo "refinement proved for the committed implementation; injected defect rejected (evidence in verification/aeneas/evidence/)"
