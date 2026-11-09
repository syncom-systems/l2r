#!/usr/bin/env bash
# The composite/v1 proof lane: the GENERATED Rust scheduler refines the Lean scheduler model.
#
#   bazel test --config=verify //verification/aeneas_v1:refinement_test
#
# Gates, in order. Any failure fails the test.
#   1. extract     Charon + Aeneas translate the committed generated crate
#                  (examples/crossing_v1/scaffold, via crossing_v1_core.rs); the result is
#                  byte-identical to the committed translation;
#   2. prove       lean/L2R, the Lean spec, the derived network, AeneasV1/Model.lean (kernel
#                  `decide` over every well-formed scheduler state), the translation and
#                  Refinement.lean compile;
#   3. audit       every expected theorem exists, depends only on propext / Classical.choice /
#                  Quot.sound, and its statement and every abstraction function equal the
#                  reviewed expected-statements.txt;
#   4. kernel      leanchecker re-checks every declaration of the lane's own modules;
#   5. mutants     each seeded defect in the generated Rust (mutants.tsv) makes its named
#                  theorem fail, and no other;
#   6. bypasses    each attempt in bypasses.tsv to pass without a real proof is rejected by its
#                  named gate.
# Evidence goes to $TEST_UNDECLARED_OUTPUTS_DIR (bazel-testlogs/.../outputs).
set -euo pipefail

manifest="$1"
if ! grep -q '^status=available' "$manifest"; then
  echo "FAIL: verification toolchain unavailable: $(sed -n 's/^reason=//p' "$manifest")" >&2
  echo "run: bazel test --config=verify //verification/aeneas_v1:refinement_test" >&2
  exit 1
fi
# Provides $TOOLS $LEAN $LEAN_LIBS $CHARON $AENEAS $DYLIB_VAR, apply_edit, theorem_range, sha256.
source verification/aeneas/toolchain.sh "$manifest"
source verification/aeneas_v1/lane.sh

evidence="${TEST_UNDECLARED_OUTPUTS_DIR:-$TEST_TMPDIR/evidence}"
mkdir -p "$evidence"
cp "$manifest" "$evidence/toolchain-manifest.txt"
ws="$TEST_TMPDIR/ws"
v1_copy_sources "$ws"
cd "$ws"

step() { echo "== $*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

step "1. extraction from the committed generated Rust"
v1_extract "$ws" "$TEST_TMPDIR/extracted" > "$evidence/extract.log" 2>&1 || { cat "$evidence/extract.log" >&2; fail "extraction failed"; }
cp "$TEST_TMPDIR/extracted/CrossingV1Core.lean" "$evidence/CrossingV1Core.lean"
committed=verification/aeneas_v1/proof/Extracted/CrossingV1Core.lean
cmp -s "$TEST_TMPDIR/extracted/CrossingV1Core.lean" "$committed" || {
  diff "$committed" "$TEST_TMPDIR/extracted/CrossingV1Core.lean" > "$evidence/stale-translation.diff" || true
  fail "the committed translation is stale; run: bazel run --config=verify //verification/aeneas_v1:update_translation"
}
{
  for f in $V1_RUST_SOURCES; do sha256 "$f"; done
  sha256 "$committed"
} > "$evidence/source-identity.txt"

step "2. proof checking (Lean model bridge, translation, refinement)"
olean="$TEST_TMPDIR/olean"
v1_compile_all "$ws" "$olean" > "$evidence/prove.log" 2>&1 || { cat "$evidence/prove.log" >&2; fail "the refinement proof does not check"; }

step "3. audit (axioms, statements, abstraction functions)"
v1_audit "$ws" "$olean" > "$evidence/audit.log" 2>&1 || { cat "$evidence/audit.log" >&2; fail "the audit failed"; }
grep -q "AUDIT PASSED" "$evidence/audit.log" || fail "the audit did not report success"
grep -E "^(DEFINITION|CONSTRUCTOR|STATEMENT) " "$evidence/audit.log" > "$evidence/statements.txt" || true
cmp -s "$evidence/statements.txt" verification/aeneas_v1/expected-statements.txt || {
  diff verification/aeneas_v1/expected-statements.txt "$evidence/statements.txt" >&2 || true
  fail "proved statements differ from the reviewed expected-statements.txt"
}

step "4. kernel replay of the lane's modules (leanchecker)"
v1_kernel_check "$olean" > "$evidence/kernel.log" 2>&1 || { cat "$evidence/kernel.log" >&2; fail "leanchecker rejected a declaration"; }
echo "leanchecker: every declaration of the lane's modules re-checked by the kernel" | tee "$evidence/kernel.txt"

step "5. seeded defects in the generated Rust are detected by the named theorem"
v1_run_mutants "$ws" "$olean" "$evidence"

step "6. proof bypasses are rejected"
v1_run_bypasses "$ws" "$olean" "$evidence"

echo "PASS: translation current, proofs checked, audit clean, all mutants and bypasses rejected" | tee "$evidence/summary.txt"
