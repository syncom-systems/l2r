#!/usr/bin/env bash
# End to end: a new project in a new Bazel workspace, outside this repository.
#
#   tools/e2e/new_project.sh            (KEEP=1 keeps the workspace for inspection)
#
# Follows docs/getting-started.md step by step, using L2R only as an external
# Bazel dependency (`@l2r`), and checks every refusal on the way:
#
#   init -> (inventory) -> check fails: nothing approved -> preview -> approve ->
#   generate -> tests fail: nothing implemented -> implement -> tests pass ->
#   change the model -> check fails: stale approval, generate refuses ->
#   re-approve, regenerate: implementation untouched -> edit a generated file ->
#   check fails: drift -> work packet.
#
# The implementation step copies the committed implementation of
# examples/starter (the same system under its template names).
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"
if [ "${KEEP:-}" = 1 ]; then echo "workspace: $work"; else trap 'rm -rf "$work"' EXIT; fi
proj="$work/pump_station"
log="$work/log.txt"

step() { echo "== $*"; }
fail() { echo "FAIL: $*" >&2; [ -f "$log" ] && tail -40 "$log" >&2; exit 1; }
# refused <pattern> <command...>: the command must fail and say <pattern>.
refused() {
  local pattern="$1"; shift
  if "$@" > "$log" 2>&1; then fail "expected a refusal ($pattern) from: $*"; fi
  grep -q -- "$pattern" "$log" || fail "refused, but not with \`$pattern\`: $*"
}

step "1. l2r init: a new workspace"
(cd "$repo" && bazel run //crates/l2r_cli:l2r -- init "$proj" --id pump_station --workspace-path "$repo") > "$log" 2>&1 \
  || fail "init failed"
refused "already exists" bash -c "cd '$repo' && bazel run //crates/l2r_cli:l2r -- init '$proj' --id pump_station"

cd "$proj"
bazel build @l2r//crates/l2r_cli:l2r > "$log" 2>&1 || fail "L2R does not build as an external dependency"
L2R="$(bazel cquery --output=files @l2r//crates/l2r_cli:l2r 2>/dev/null)"
L2R="$(bazel info execution_root 2>/dev/null)/$L2R"
[ -x "$L2R" ] || fail "cannot locate the l2r binary"
M="$proj/architecture.toml"

"$L2R" validate "$M" > "$log" 2>&1 || fail "the new project's model is not valid"

step "2. the inventory init wrote is what its Lean specification exports"
if command -v lake > /dev/null; then
  "$L2R" inventory "$proj/spec" --out "$work/fresh.json" PumpStation > "$log" 2>&1 || fail "l2r inventory failed"
  cmp -s "$work/fresh.json" "$proj/lean-inventory.json" || fail "the inventory written by init differs from a fresh export"
  rm -rf "$proj/spec/.lake"
  echo "fresh export equals the committed inventory"
else
  echo "skipped: \`lake\` is not installed (the proof lane checks this with the pinned Lean)"
fi

step "3. nothing approved yet: the drift check fails and says what to do"
refused "no approval record yet" bazel test //...
refused "no approval recorded" "$L2R" generate "$M"

step "4. preview, approve exactly that plan, generate"
"$L2R" preview "$M" --summary > "$work/preview.txt" || fail "preview failed"
digest="$(sed -n 's/^Plan digest: //p' "$work/preview.txt")"
refused "approval refused" "$L2R" approve "$M" --digest sha256:0000 --by "E2E"
"$L2R" approve "$M" --digest "$digest" --by "E2E Architect" > "$log" 2>&1 || fail "approve failed"
"$L2R" generate "$M" > "$log" 2>&1 || fail "generate failed"

step "5. nothing implemented yet: the contract tests fail"
refused "not yet implemented" bazel test --test_output=errors //...

step "6. implement inside the generated contracts"
src="$repo/examples/starter/scaffold/src/starter"
dst="$proj/scaffold/src/pump_station"
for f in monitor/imp.rs routes.rs; do
  sed -e 's/Starter/PumpStation/g' -e 's/starter/pump_station/g' "$src/$f" > "$dst/$f"
done
bazel test //... > "$log" 2>&1 || fail "the implemented project does not pass its tests"
cp "$dst/monitor/imp.rs" "$work/imp.saved"

step "7. change the architecture: the approval is stale until the new plan is reviewed"
sed -i.bak 's/^responsibility = "Watches readings/responsibility = "Observes readings/' "$M" && rm "$M.bak"
refused "architecture model changed" bazel test --test_output=errors //:scaffold_matches_approved_plan_test
refused "generation refused" "$L2R" generate "$M"
digest="$("$L2R" preview "$M" --summary | sed -n 's/^Plan digest: //p')"
"$L2R" approve "$M" --digest "$digest" --by "E2E Architect" > "$log" 2>&1 || fail "re-approve failed"
"$L2R" generate "$M" > "$log" 2>&1 || fail "regenerate failed"
cmp -s "$dst/monitor/imp.rs" "$work/imp.saved" || fail "regeneration changed an implementation file"
grep -q "Responsibility: Observes readings" "$dst/monitor/mod.rs" || fail "the regenerated contract does not reflect the model"
bazel test //... > "$log" 2>&1 || fail "tests fail after regeneration"

step "8. an edited generated file is drift"
cp "$dst/mod.rs" "$work/mod.saved"
echo "// a helpful tweak" >> "$dst/mod.rs"
refused "edited after generation" bazel test --test_output=errors //:scaffold_matches_approved_plan_test
cp "$work/mod.saved" "$dst/mod.rs"
bazel test //... > "$log" 2>&1 || fail "tests fail after restoring the generated file"

step "9. the work packet"
"$L2R" packet "$M" monitor > "$work/packet.md" || fail "packet failed"
grep -q "scaffold/src/pump_station/monitor/imp.rs" "$work/packet.md" || fail "the packet does not name the implementation file"
grep -q "PumpStation.Monitor.alert_only_above_limit" "$work/packet.md" || fail "the packet does not list the theorems"

echo "E2E PASS: a new project, built with L2R as an external dependency, went through the whole workflow"
