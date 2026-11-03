#!/usr/bin/env bash
# The composition lane: system-level proofs over every permitted interleaving.
#
#   bazel test --config=verify //verification/composition:composition_test
#
# Gates, in order. Any failure fails the test.
#   1. prove     the generic semantics (lean/L2R/Async.lean), the Lean spec, the
#                derived composition (examples/crossing/composition/L2RDerived)
#                and the system proofs compile with the pinned Lean;
#   2. audit     every expected theorem exists and uses only standard axioms; its
#                statement and every definition it depends on equal the
#                reviewed expected-statements.txt;
#   3. kernel    `leanchecker --fresh` replays the system proofs and everything
#                they import, Lean's core library included, through the kernel,
#                so no compiled .olean is trusted for this lane; a fixture whose
#                proof skipped the kernel (and has no axioms) must be rejected;
#   4. mutants   each seeded deviation in mutants.tsv (of the scheduler model, the
#                spec or the environment) is rejected inside its named theorems,
#                or, if marked `accepted`, still proves the same statements;
#   5. bypasses  each attempt in bypasses.tsv to pass without a real proof is
#                rejected by its named gate;
#   6. conform   bounded testing, not proof: the generated Rust scheduler and the
#                Lean scheduler model agree on every operation sequence of length
#                $CONFORMANCE_LENGTH (scheduler_traces + Conformance.lean). An
#                `accepted` scheduler mutant (a different but permitted order)
#                must disagree with the Rust here.
#
# The derived composition's freshness (it equals `l2r lean-composition` of the
# committed model and inventory) is checked by
# //crates/l2r_plan:l2r_plan_lean_composition_test in the ordinary test run.
# Evidence goes to $TEST_UNDECLARED_OUTPUTS_DIR (bazel-testlogs/.../outputs).
set -euo pipefail

manifest="$1"
traces_bin="$(realpath "$2")"
CONFORMANCE_LENGTH=7
if ! grep -q '^status=available' "$manifest"; then
  echo "FAIL: verification toolchain unavailable: $(sed -n 's/^reason=//p' "$manifest")" >&2
  echo "run: bazel test --config=verify //verification/composition:composition_test" >&2
  exit 1
fi
# Provides $LEAN, $TOOLS, apply_edit, theorem_range and sha256.
source verification/aeneas/toolchain.sh "$manifest"

evidence="${TEST_UNDECLARED_OUTPUTS_DIR:-$TEST_TMPDIR/evidence}"
mkdir -p "$evidence"
cp "$manifest" "$evidence/toolchain-manifest.txt"

step() { echo "== $*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

LANE=verification/composition
ws="$TEST_TMPDIR/ws"
mkdir -p "$ws"
for f in $(find lean examples/crossing/spec examples/crossing/composition "$LANE" \( -type f -o -type l \) -name '*.lean') \
         "$LANE/expected-statements.txt" "$LANE/mutants.tsv" "$LANE/bypasses.tsv"; do
  mkdir -p "$ws/$(dirname "$f")"
  cp -L "$f" "$ws/$f"
done

# Module roots and modules, in dependency order. Only the pinned Lean and these
# sources are on the path: the lane imports no mathlib and no Aeneas library.
MODULES="lean L2R/Async.lean
examples/crossing/spec Crossing/Latch.lean
examples/crossing/spec Crossing/Signal.lean
examples/crossing/spec Crossing/Routes.lean
examples/crossing/spec Crossing.lean
examples/crossing/composition L2RDerived/crossing.lean
examples/crossing/composition CrossingProperties.lean"

# build <workspace> <out>: compiles every module; stops at the first failure.
build() {
  local w="$1" out="$2" root file
  while read -r root file; do
    mkdir -p "$(dirname "$out/${file%.lean}.olean")"
    LEAN_PATH="$out" "$LEAN" -R "$w/$root" "$w/$root/$file" -o "$out/${file%.lean}.olean" || return 1
  done <<< "$MODULES"
}

# audit <workspace> <out>: prints the audit; succeeds only if it passed.
audit() {
  local w="$1" out="$2"
  LEAN_PATH="$out" "$LEAN" -R "$w/$LANE" "$w/$LANE/Audit.lean" && return 0
  return 1
}

pinned_lines() { grep -oE '(STATEMENT|DEFINITION|CONSTRUCTOR) .*' "$1" || true; }

# conform <workspace> <out> <log>: replays the recorded Rust traces on the Lean model.
conform() {
  LEAN_PATH="$2" "$LEAN" --run "$1/$LANE/Conformance.lean" "$TEST_TMPDIR/traces.txt" > "$3" 2>&1
}

"$traces_bin" "$CONFORMANCE_LENGTH" > "$TEST_TMPDIR/traces.txt" || fail "recording the Rust scheduler's traces failed"
cd "$ws"
{
  for f in $(echo "$MODULES" | awk '{print $1 "/" $2}') "$LANE/Audit.lean"; do sha256 "$f"; done
} > "$evidence/source-identity.txt"

step "1. proof checking"
out="$TEST_TMPDIR/olean"
build "$ws" "$out" > "$evidence/prove.log" 2>&1 || { cat "$evidence/prove.log" >&2; fail "the composition proofs do not check"; }

step "2. audit (axioms, statements, definitions)"
audit "$ws" "$out" > "$evidence/audit.log" 2>&1 || { cat "$evidence/audit.log" >&2; fail "the audit failed"; }
grep -q "AUDIT PASSED" "$evidence/audit.log" || fail "the audit did not report success"
pinned_lines "$evidence/audit.log" > "$evidence/statements.txt"
cmp -s "$evidence/statements.txt" "$LANE/expected-statements.txt" || {
  diff "$LANE/expected-statements.txt" "$evidence/statements.txt" >&2 || true
  fail "proved statements or pinned definitions differ from the reviewed expected-statements.txt"
}

step "3. kernel replay (leanchecker --fresh)"
PATH="$TOOLS/lean/bin:/usr/bin:/bin" LEAN_PATH="$out" "$TOOLS/lean/bin/leanchecker" --fresh CrossingProperties > "$evidence/leanchecker.log" 2>&1 \
  || { cat "$evidence/leanchecker.log" >&2; fail "leanchecker rejected the environment"; }
echo "leanchecker --fresh CrossingProperties: accepted (all imports replayed, Lean core included)" | tee "$evidence/kernel.txt"
# The gate must be able to fail: a theorem whose proof skipped the kernel
# compiles and uses only standard axioms, so only the replay can reject it.
LEAN_PATH="$out" "$LEAN" -R "$ws/$LANE" "$ws/$LANE/KernelForgery.lean" -o "$out/KernelForgery.olean" \
  > "$evidence/forgery.log" 2>&1 || { cat "$evidence/forgery.log" >&2; fail "the kernel forgery fixture does not compile"; }
grep -qE '^FORGERY AXIOMS \[((propext|Classical\.choice|Quot\.sound)(, )?)*\]$' "$evidence/forgery.log" \
  || fail "the forgery fixture depends on a non-standard axiom, so it would not show the audit's blind spot"
if PATH="$TOOLS/lean/bin:/usr/bin:/bin" LEAN_PATH="$out" "$TOOLS/lean/bin/leanchecker" KernelForgery >> "$evidence/forgery.log" 2>&1; then
  fail "leanchecker accepted a declaration that bypassed the kernel"
fi
grep -q "declaration type mismatch" "$evidence/forgery.log" || fail "leanchecker rejected the forgery for an unexpected reason"
echo "kernel forgery (standard axioms only, passes the audit's axiom check): rejected by leanchecker" | tee -a "$evidence/kernel.txt"

step "4. seeded deviations"
count=0
while IFS=$'\t' read -r name file from to expect; do
  [ -z "$name" ] || [ "${name:0:1}" = "#" ] && continue
  count=$((count + 1))
  mws="$TEST_TMPDIR/mutant-$name"; log="$evidence/mutant-$name.log"
  rm -rf "$mws"; cp -R "$ws" "$mws"
  apply_edit "$from" "$to" "$mws/$file" || fail "mutant $name: pattern not found in $file"
  cmp -s "$ws/$file" "$mws/$file" && fail "mutant $name: the source did not change"
  if [ "$expect" = accepted ]; then
    build "$mws" "$mws/olean" > "$log" 2>&1 || { cat "$log" >&2; fail "mutant $name was expected to be accepted"; }
    audit "$mws" "$mws/olean" >> "$log" 2>&1 || fail "mutant $name: the audit failed"
    diff <(grep '^STATEMENT' "$evidence/statements.txt") <(pinned_lines "$log" | grep '^STATEMENT') > /dev/null \
      || fail "mutant $name: accepted, but with different theorem statements"
    conform "$mws" "$mws/olean" "$evidence/mutant-$name-conformance.log" \
      && fail "mutant $name: accepted by the proofs, but conformance did not tell it from the generated Rust"
    echo "mutant $name: accepted, every theorem still proved with the same statement; differs from the generated Rust ($(grep -m1 -oE 'CONFORMANCE .*' "$evidence/mutant-$name-conformance.log"))" | tee -a "$evidence/mutants.txt"
    continue
  fi
  build "$mws" "$mws/olean" > "$log" 2>&1 && fail "mutant $name was NOT detected: the proofs still check"
  proof_file="${expect%%:*}"; theorems="${expect#*:}"
  base="$(basename "$proof_file")"
  lines="$(grep -oE "$base:[0-9]+:[0-9]+: error" "$log" | sed -E "s/.*$base:([0-9]+):.*/\1/" | sort -un)"
  [ -n "$lines" ] || fail "mutant $name: rejected, but not in $proof_file (setup error?): $(head -c 2000 "$log")"
  for line in $lines; do
    inside=no
    for theorem in ${theorems//,/ }; do
      read -r start end <<< "$(theorem_range "$mws/$proof_file" "$theorem")"
      [ -n "${start:-}" ] || fail "mutant $name: theorem $theorem not found in $proof_file"
      if [ "$line" -ge "$start" ] && [ "$line" -le "$end" ]; then inside=yes; fi
    done
    [ "$inside" = yes ] || fail "mutant $name: error at $base:$line is outside $theorems"
  done
  echo "mutant $name: rejected by $theorems (errors at $base lines $(echo $lines))" | tee -a "$evidence/mutants.txt"
done < "$LANE/mutants.tsv"
[ "$count" -gt 0 ] || fail "no mutants defined"

step "5. proof bypasses"
count=0
while IFS=$'\t' read -r name file from to gate; do
  [ -z "$name" ] || [ "${name:0:1}" = "#" ] && continue
  count=$((count + 1))
  bws="$TEST_TMPDIR/bypass-$name"; log="$evidence/bypass-$name.log"
  rm -rf "$bws"; cp -R "$ws" "$bws"
  apply_edit "$from" "$to" "$bws/$file" || fail "bypass $name: pattern not found in $file"
  if ! build "$bws" "$bws/olean" > "$log" 2>&1; then
    [ "$gate" = proof ] || fail "bypass $name: expected the audit to reject it, but the proofs did not compile"
    echo "bypass $name: rejected by proof checking" | tee -a "$evidence/bypasses.txt"
    continue
  fi
  [ "$gate" = audit ] || fail "bypass $name was NOT rejected: the proofs still compile"
  if audit "$bws" "$bws/olean" >> "$log" 2>&1 && grep -q "AUDIT PASSED" "$log" \
     && pinned_lines "$log" | cmp -s - "$bws/$LANE/expected-statements.txt"; then
    fail "bypass $name was NOT rejected: the audit accepted it"
  fi
  reason="$(grep -oE 'AUDIT FAILED: .*' "$log" | head -1 || true)"
  echo "bypass $name: rejected by the audit (${reason:-statement or definition mismatch})" | tee -a "$evidence/bypasses.txt"
done < "$LANE/bypasses.tsv"
[ "$count" -gt 0 ] || fail "no bypasses defined"

step "6. bounded conformance of the generated Rust scheduler with the Lean model"
conform "$ws" "$out" "$evidence/conformance.log" || { head -20 "$evidence/conformance.log" >&2; fail "the generated Rust scheduler and the Lean scheduler model disagree"; }
grep -m1 -oE 'CONFORMANCE .*' "$evidence/conformance.log" | sed "s/\$/ (every sequence of $CONFORMANCE_LENGTH operations)/" | tee "$evidence/conformance.txt"

echo "PASS: composition proofs checked, audited, kernel-replayed; all deviations and bypasses handled as expected" | tee "$evidence/summary.txt"
