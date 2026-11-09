#!/usr/bin/env bash
# The composite/v2 lane: hierarchical composition and output ports.
#
#   bazel test --config=verify //verification/composition_v2:composition_v2_test
#
# Gates, in order. Any failure fails the test.
#   0. identity  each derived flat network names the model, inventory and
#                generator of its approval record, and the exact semantics and
#                scheduler-model sources it imports; seeded mismatches are rejected;
#   1. prove     the semantics (lean/L2R/Async1.lean), the scheduler model with
#                output ports (lean/L2R/Async1Ports.lean), the Lean specs and the
#                derived networks compile with the pinned Lean;
#   2. audit     every expected theorem exists and uses only standard axioms; its
#                statement and every pinned definition (scheduler model, derived
#                routes, capacities, ports) equal the reviewed expected-statements.txt;
#   3. kernel    `leanchecker --fresh` replays everything, Lean's core library
#                included; a fixture whose proof skipped the kernel must be rejected;
#   4. inventory the committed `l2r.lean-inventory/v1` files equal a fresh export
#                from the compiled specifications; and what `l2r init` writes
#                for a project under other names (it renames the starter's
#                specification and inventory) is again a fresh export;
#   5. mutants   each seeded deviation of the scheduler model in mutants.tsv is
#                rejected inside its named theorems;
#   6. bypasses  each attempt in bypasses.tsv to pass without a real proof is
#                rejected by its named gate;
#   7. conform   bounded testing, not proof: for every example the generated Rust
#                scheduler and the Lean scheduler model agree on every operation
#                sequence of length $EXHAUSTIVE and on $RANDOM_COUNT pseudo-random
#                sequences of length $RANDOM_LENGTH (the derived harness of
#                `l2r conformance`). A scheduler model with a different but
#                still permitted order must disagree with the Rust here.
#
# The derived networks' freshness (each equals `l2r lean-composition` of the
# committed model and inventory) is checked by //crates/l2r_plan:l2r_plan_v2_test
# in the ordinary test run. Evidence goes to $TEST_UNDECLARED_OUTPUTS_DIR.
set -euo pipefail

manifest="$1"
l2r="$(realpath "$2")"
traces_tank_station="$(realpath "$3")"
traces_relay="$(realpath "$4")"
traces_starter="$(realpath "$5")"
EXHAUSTIVE=4
RANDOM_COUNT=3000
RANDOM_LENGTH=80
if ! grep -q '^status=available' "$manifest"; then
  echo "FAIL: verification toolchain unavailable: $(sed -n 's/^reason=//p' "$manifest")" >&2
  echo "run: bazel test --config=verify //verification/composition_v2:composition_v2_test" >&2
  exit 1
fi
# Provides $LEAN, $TOOLS, apply_edit, theorem_range and sha256.
source verification/aeneas/toolchain.sh "$manifest"

evidence="${TEST_UNDECLARED_OUTPUTS_DIR:-$TEST_TMPDIR/evidence}"
mkdir -p "$evidence"
cp "$manifest" "$evidence/toolchain-manifest.txt"

step() { echo "== $*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

LANE=verification/composition_v2
EXAMPLES="tank_station relay starter"
ws="$TEST_TMPDIR/ws"
mkdir -p "$ws"
for f in $(find lean examples/tank_station/spec examples/relay/spec examples/starter/spec examples/tank_station/composition \
             examples/relay/composition examples/starter/composition \
             examples/tank_station/conformance examples/relay/conformance examples/starter/conformance \
             "$LANE" tools/lean_inventory \( -type f -o -type l \) -name '*.lean') \
         "$LANE/expected-statements.txt" "$LANE/mutants.tsv" "$LANE/bypasses.tsv" \
         examples/tank_station/architecture.approval.json examples/relay/architecture.approval.json \
         examples/starter/architecture.approval.json examples/starter/lean-inventory.json \
         examples/tank_station/lean-inventory.json examples/relay/lean-inventory.json; do
  mkdir -p "$ws/$(dirname "$f")"
  cp -L "$f" "$ws/$f"
done

# Module roots and modules, in dependency order. Only the pinned Lean and these
# sources are on the path: the lane imports no mathlib and no Aeneas library.
MODULES="lean L2R/Async.lean
lean L2R/Async1.lean
lean L2R/Async1Ports.lean
examples/tank_station/spec Tank/Common.lean
examples/tank_station/spec Tank/Filter.lean
examples/tank_station/spec Tank/Controller.lean
examples/tank_station/spec Tank/Alarm.lean
examples/tank_station/spec Tank/Ports.lean
examples/tank_station/spec Tank/Routes.lean
examples/tank_station/spec Tank.lean
examples/relay/spec Relay/Machines.lean
examples/relay/spec Relay/Wiring.lean
examples/relay/spec Relay.lean
examples/starter/spec Starter/Monitor.lean
examples/starter/spec Starter/Wiring.lean
examples/starter/spec Starter.lean
examples/tank_station/composition L2RDerived/tank_station.lean
examples/relay/composition L2RDerived/relay.lean
examples/starter/composition L2RDerived/starter.lean"

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

cd "$ws"
{
  for f in $(echo "$MODULES" | awk '{print $1 "/" $2}') "$LANE/Audit.lean"; do sha256 "$f"; done
} > "$evidence/source-identity.txt"

step "0. source identity (each derived network is about the approved architecture)"
# check_identity <derived> <approval> <semantics> <scheduler>
check_identity() {
  local d="$1" a="$2" m="$3" p="$4" sha psha
  header_of() { sed -n "s/^-- $1: //p" "$d" | head -1; }
  approved() { sed -n "s/^  \"$1\": \"\(.*\)\",\{0,1\}$/\1/p" "$a" | head -1; }
  sha="sha256:$(sha256 "$m" | cut -d' ' -f1)"
  psha="sha256:$(sha256 "$p" | cut -d' ' -f1)"
  echo "derived file          $d"
  echo "generator             $(header_of generator)"
  echo "semantics             $(header_of semantics)"
  echo "scheduler             $(header_of scheduler)"
  echo "model                 $(header_of model)"
  echo "inventory             $(header_of inventory)"
  echo "approval plan digest  $(approved plan_digest)"
  [ -n "$(header_of model)" ] && [ -n "$(approved model_digest)" ] || { echo "identity: cannot read the derived header or the approval record"; return 1; }
  [ "$(header_of model)" = "$(approved model_digest)" ] || { echo "identity: the derived network is not derived from the approved model"; return 1; }
  [ "$(header_of inventory)" = "$(approved inventory_digest)" ] || { echo "identity: the derived network is not derived from the approved Lean inventory"; return 1; }
  [ "$(header_of generator)" = "$(approved generator)" ] || { echo "identity: the derived network was produced by a different generator than the approval names"; return 1; }
  [ "$(header_of semantics)" = "l2r.async/v1 $sha" ] || { echo "identity: the derived network names a different semantics source than $m"; return 1; }
  [ "$(header_of scheduler)" = "l2r.async1-ports/v1 $psha" ] || { echo "identity: the derived network names a different scheduler model than $p"; return 1; }
}
semantics_file=lean/L2R/Async1.lean
scheduler_file=lean/L2R/Async1Ports.lean
for example in $EXAMPLES; do
  derived="examples/$example/composition/L2RDerived/$example.lean"
  approval="examples/$example/architecture.approval.json"
  check_identity "$derived" "$approval" "$semantics_file" "$scheduler_file" > "$evidence/identity-$example.txt" \
    || { cat "$evidence/identity-$example.txt" >&2; fail "the derived network of $example does not match the approved architecture"; }
  cat "$evidence/identity-$example.txt"
done
# The gate must be able to fail: each seeded inconsistency is rejected.
derived=examples/tank_station/composition/L2RDerived/tank_station.lean
approval=examples/tank_station/architecture.approval.json
idws="$TEST_TMPDIR/identity"
for seeded in model inventory generator semantics scheduler-source; do
  rm -rf "$idws"; mkdir -p "$idws"
  cp "$derived" "$idws/derived.lean"; cp "$approval" "$idws/approval.json"
  cp "$semantics_file" "$idws/Async1.lean"; cp "$scheduler_file" "$idws/Async1Ports.lean"
  case "$seeded" in
    model)            sed -i.bak 's/^-- model: sha256:./-- model: sha256:0/' "$idws/derived.lean" ;;
    inventory)        sed -i.bak 's/^-- inventory: sha256:./-- inventory: sha256:0/' "$idws/derived.lean" ;;
    generator)        sed -i.bak 's/^-- generator: l2r/-- generator: other/' "$idws/derived.lean" ;;
    semantics)        echo "-- edited after derivation" >> "$idws/Async1.lean" ;;
    scheduler-source) echo "-- edited after derivation" >> "$idws/Async1Ports.lean" ;;
  esac
  if check_identity "$idws/derived.lean" "$idws/approval.json" "$idws/Async1.lean" "$idws/Async1Ports.lean" > "$evidence/identity-$seeded.log" 2>&1; then
    fail "identity gate accepted a seeded inconsistency: $seeded"
  fi
  grep -q '^identity:' "$evidence/identity-$seeded.log" || fail "identity gate rejected $seeded for an unexpected reason"
  echo "identity mismatch ($seeded): rejected" | tee -a "$evidence/identity-negative.txt"
done

step "1. proof checking"
out="$TEST_TMPDIR/olean"
build "$ws" "$out" > "$evidence/prove.log" 2>&1 || { cat "$evidence/prove.log" >&2; fail "the scheduler model or a derived network does not check"; }

step "2. audit (axioms, statements, definitions)"
audit "$ws" "$out" > "$evidence/audit.log" 2>&1 || { cat "$evidence/audit.log" >&2; fail "the audit failed"; }
grep -q "AUDIT PASSED" "$evidence/audit.log" || fail "the audit did not report success"
pinned_lines "$evidence/audit.log" > "$evidence/statements.txt"
cmp -s "$evidence/statements.txt" "$LANE/expected-statements.txt" || {
  diff "$LANE/expected-statements.txt" "$evidence/statements.txt" >&2 || true
  fail "proved statements or pinned definitions differ from the reviewed expected-statements.txt"
}

step "3. kernel replay (leanchecker --fresh)"
for module in L2RDerived.tank_station L2RDerived.relay L2RDerived.starter; do
  PATH="$TOOLS/lean/bin:/usr/bin:/bin" LEAN_PATH="$out" "$TOOLS/lean/bin/leanchecker" --fresh "$module" > "$evidence/leanchecker-$module.log" 2>&1 \
    || { cat "$evidence/leanchecker-$module.log" >&2; fail "leanchecker rejected the environment of $module"; }
  echo "leanchecker --fresh $module: accepted (all imports replayed, Lean core included)" | tee -a "$evidence/kernel.txt"
done
LEAN_PATH="$out" "$LEAN" -R "$ws/$LANE" "$ws/$LANE/KernelForgery.lean" -o "$out/KernelForgery.olean" \
  > "$evidence/forgery.log" 2>&1 || { cat "$evidence/forgery.log" >&2; fail "the kernel forgery fixture does not compile"; }
grep -qE '^FORGERY AXIOMS \[((propext|Classical\.choice|Quot\.sound)(, )?)*\]$' "$evidence/forgery.log" \
  || fail "the forgery fixture depends on a non-standard axiom, so it would not show the audit's blind spot"
if PATH="$TOOLS/lean/bin:/usr/bin:/bin" LEAN_PATH="$out" "$TOOLS/lean/bin/leanchecker" KernelForgery >> "$evidence/forgery.log" 2>&1; then
  fail "leanchecker accepted a declaration that bypassed the kernel"
fi
grep -q "declaration type mismatch" "$evidence/forgery.log" || fail "leanchecker rejected the forgery for an unexpected reason"
echo "kernel forgery (standard axioms only, passes the audit's axiom check): rejected by leanchecker" | tee -a "$evidence/kernel.txt"

step "4. inventory freshness (l2r.lean-inventory/v1)"
for pair in "tank_station Tank" "relay Relay" "starter Starter"; do
  read -r example module <<< "$pair"
  PATH="$TOOLS/lean/bin:$PATH" LEAN_PATH="$out" "$LEAN" --run tools/lean_inventory/Main.lean --v1 "$module" > "$evidence/inventory-$example.json" \
    || fail "exporting the inventory of $example failed"
  cmp -s "$evidence/inventory-$example.json" "examples/$example/lean-inventory.json" || {
    diff "examples/$example/lean-inventory.json" "$evidence/inventory-$example.json" | head -20 >&2 || true
    fail "examples/$example/lean-inventory.json is not what the compiled specification exports"
  }
  echo "inventory of $example: equals a fresh export" | tee -a "$evidence/inventory.txt"
done

# `l2r init` renames the starter. Its output must again be a specification that
# compiles and an inventory that is exactly what that specification exports.
init="$TEST_TMPDIR/init"
rm -rf "$init"
"$l2r" init "$init" --id pump_station > "$evidence/init.log" 2>&1 || { cat "$evidence/init.log" >&2; fail "l2r init failed"; }
"$l2r" validate "$init/architecture.toml" >> "$evidence/init.log" 2>&1 || { cat "$evidence/init.log" >&2; fail "the project written by l2r init is not valid"; }
initout="$TEST_TMPDIR/init-olean"
for m in PumpStation/Monitor PumpStation/Wiring PumpStation; do
  mkdir -p "$(dirname "$initout/$m.olean")"
  LEAN_PATH="$initout" "$LEAN" -R "$init/spec" "$init/spec/$m.lean" -o "$initout/$m.olean" >> "$evidence/init.log" 2>&1 \
    || { cat "$evidence/init.log" >&2; fail "the specification written by l2r init does not compile"; }
done
PATH="$TOOLS/lean/bin:$PATH" LEAN_PATH="$initout" "$LEAN" --run tools/lean_inventory/Main.lean --v1 PumpStation > "$evidence/inventory-init.json" \
  || fail "exporting the inventory of the project written by l2r init failed"
cmp -s "$evidence/inventory-init.json" "$init/lean-inventory.json" || fail "the inventory written by l2r init is not what its specification exports"
echo "inventory written by \`l2r init --id pump_station\`: equals a fresh export of the renamed specification" | tee -a "$evidence/inventory.txt"

step "5. seeded deviations"
count=0
while IFS=$'\t' read -r name file from to expect; do
  [ -z "$name" ] || [ "${name:0:1}" = "#" ] && continue
  count=$((count + 1))
  mws="$TEST_TMPDIR/mutant-$name"; log="$evidence/mutant-$name.log"
  rm -rf "$mws"; cp -R "$ws" "$mws"
  apply_edit "$from" "$to" "$mws/$file" || fail "mutant $name: pattern not found in $file"
  cmp -s "$ws/$file" "$mws/$file" && fail "mutant $name: the source did not change"
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

step "6. proof bypasses"
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

step "7. bounded conformance of the generated Rust schedulers with the Lean model"
# conform <example> <olean dir> <workspace> <log>
conform() {
  LEAN_PATH="$2" "$LEAN" --run "$3/examples/$1/conformance/Conformance.lean" "$TEST_TMPDIR/traces-$1.txt" > "$4" 2>&1
}
for example in $EXAMPLES; do
  recorder="traces_$example"
  "${!recorder}" "$EXHAUSTIVE" "$RANDOM_COUNT" "$RANDOM_LENGTH" > "$TEST_TMPDIR/traces-$example.txt" \
    || fail "recording the Rust scheduler's traces of $example failed"
  conform "$example" "$out" "$ws" "$evidence/conformance-$example.log" \
    || { head -20 "$evidence/conformance-$example.log" >&2; fail "the generated Rust scheduler of $example and the Lean scheduler model disagree"; }
  echo "$example: $(grep -m1 -oE 'CONFORMANCE .*' "$evidence/conformance-$example.log") (every sequence of $EXHAUSTIVE operations; $RANDOM_COUNT pseudo-random sequences of $RANDOM_LENGTH)" | tee -a "$evidence/conformance.txt"
done
# The gate must be able to fail. A scheduler that keeps serving the machine it
# just served is still sound (every theorem about it holds), but it is not the
# order the generated Rust uses: conformance must tell them apart.
cws="$TEST_TMPDIR/conformance-mutant"
rm -rf "$cws"; cp -R "$ws" "$cws"
apply_edit '(⟨k, (c.val + 1) % machines⟩ : Sched N)' '(⟨k, c.val⟩ : Sched N)' "$cws/lean/L2R/Async1Ports.lean" \
  || fail "conformance mutant: pattern not found"
build "$cws" "$cws/olean" > "$evidence/conformance-mutant.log" 2>&1 \
  || { cat "$evidence/conformance-mutant.log" >&2; fail "the sticky-cursor scheduler was expected to satisfy every theorem"; }
if conform relay "$cws/olean" "$cws" "$evidence/conformance-mutant-relay.log"; then
  fail "conformance did not tell a scheduler with a different order from the generated Rust"
fi
echo "sticky-cursor scheduler (every theorem still holds): rejected by conformance on relay ($(grep -m1 -oE 'CONFORMANCE .*' "$evidence/conformance-mutant-relay.log"))" | tee -a "$evidence/conformance.txt"

echo "PASS: scheduler model and derived networks checked, audited, kernel-replayed; inventories fresh; all deviations and bypasses rejected; generated schedulers conform on the sequences tried" | tee "$evidence/summary.txt"
