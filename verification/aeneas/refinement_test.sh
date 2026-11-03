#!/usr/bin/env bash
# The proof lane: source-to-proof verification of the crossing example.
#
#   bazel test --config=verify //verification/aeneas:refinement_test
#
# Gates, in order. Any failure fails the test.
#   1. inventory   the committed Lean inventory equals a fresh export of the spec;
#   2. extract     Charon + Aeneas translate the committed Rust sources, and the
#                  result is byte-identical to the committed translation;
#   3. prove       the spec, the translation, Refinement.lean and Audit.lean compile;
#   4. audit       every expected theorem exists, uses only standard axioms, and
#                  its statement and the abstraction functions equal the
#                  reviewed expected-statements.txt;
#   5. mutants     each seeded defect in the Rust sources makes its named
#                  refinement theorem fail, and only that theorem.
# Evidence goes to $TEST_UNDECLARED_OUTPUTS_DIR (bazel-testlogs/.../outputs).
set -euo pipefail

manifest="$1"
if ! grep -q '^status=available' "$manifest"; then
  echo "FAIL: verification toolchain unavailable: $(sed -n 's/^reason=//p' "$manifest")" >&2
  echo "run: bazel test --config=verify //verification/aeneas:refinement_test" >&2
  exit 1
fi
# Both `bazel run` and `bazel test` start in the runfiles root.
source verification/aeneas/toolchain.sh "$manifest"

evidence="${TEST_UNDECLARED_OUTPUTS_DIR:-$TEST_TMPDIR/evidence}"
mkdir -p "$evidence"
cp "$manifest" "$evidence/toolchain-manifest.txt"
ws="$TEST_TMPDIR/ws"
copy_sources "$ws"
cd "$ws"

step() { echo "== $*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

step "1. inventory freshness"
spec_out="$TEST_TMPDIR/olean-spec"
compile_spec "$ws" "$spec_out"
PATH="$TOOLS/lean/bin:$PATH" LEAN_PATH="$LEAN_LIBS:$spec_out" "$LEAN" --run tools/lean_inventory/Main.lean Crossing > "$evidence/inventory.json"
cmp -s "$evidence/inventory.json" examples/crossing/lean-inventory.json \
  || { diff examples/crossing/lean-inventory.json "$evidence/inventory.json" >&2 || true; fail "examples/crossing/lean-inventory.json is stale"; }

step "2. extraction from the committed Rust sources"
extract "$ws" "$TEST_TMPDIR/extracted" > "$evidence/extract.log" 2>&1 || { cat "$evidence/extract.log" >&2; fail "extraction failed"; }
cp "$TEST_TMPDIR/extracted/CrossingCore.lean" "$evidence/CrossingCore.lean"
committed=verification/aeneas/proof/Extracted/CrossingCore.lean
cmp -s "$TEST_TMPDIR/extracted/CrossingCore.lean" "$committed" || {
  diff "$committed" "$TEST_TMPDIR/extracted/CrossingCore.lean" > "$evidence/stale-translation.diff" || true
  fail "the committed translation is stale; run: bazel run --config=verify //verification/aeneas:update_translation"
}
{
  for f in $RUST_SOURCES; do sha256 "$f"; done
  sha256 "$committed"
} > "$evidence/source-identity.txt"

step "3. proof checking"
proof_out="$TEST_TMPDIR/olean-proof"
compile_proof "$ws" "$spec_out" "$proof_out" > "$evidence/prove.log" 2>&1 || { cat "$evidence/prove.log" >&2; fail "the refinement proof does not check"; }

step "4. audit (axioms, statements, abstraction functions)"
audit_proof "$ws" "$spec_out" "$proof_out" > "$evidence/audit.log" 2>&1 || { cat "$evidence/audit.log" >&2; fail "the audit failed"; }
grep -q "AUDIT PASSED" "$evidence/audit.log" || fail "the audit did not report success"
grep -E "^(DEFINITION|STATEMENT) " "$evidence/audit.log" > "$evidence/statements.txt" || true
cmp -s "$evidence/statements.txt" verification/aeneas/expected-statements.txt || {
  diff verification/aeneas/expected-statements.txt "$evidence/statements.txt" >&2 || true
  fail "proved statements differ from the reviewed expected-statements.txt"
}

step "5. seeded defects are detected by the named theorem"
run_mutants "$ws" "$spec_out" "$evidence"

echo "PASS: translation current, proofs checked, audit clean, all mutants rejected" | tee "$evidence/summary.txt"
