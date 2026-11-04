# Functions of the composite/v1 proof lane. Sourced after verification/aeneas/toolchain.sh,
# which provides $TOOLS $LEAN $LEAN_LIBS $CHARON $AENEAS $DYLIB_VAR and apply_edit/theorem_range.
# shellcheck shell=bash

V1_RUST_SOURCES="$(cd "${RUNFILES_ROOT:-$PWD}" && find examples/crossing_v1/scaffold/src -name '*.rs' ! -name contract_tests.rs | sort | tr '\n' ' ')"

# Module roots and modules in dependency order: "<root> <file>".
V1_MODULES="lean L2R/Async.lean
lean L2R/Async1.lean
examples/crossing_v1/spec CrossingV1/Latch.lean
examples/crossing_v1/spec CrossingV1/Signal.lean
examples/crossing_v1/spec CrossingV1/Ports.lean
examples/crossing_v1/spec CrossingV1/Routes.lean
examples/crossing_v1/spec CrossingV1.lean
examples/crossing_v1/composition L2RDerived/crossing_v1.lean
verification/aeneas_v1/proof AeneasV1/Model.lean
verification/aeneas_v1/proof Extracted/CrossingV1Core.lean
verification/aeneas_v1/proof Refinement.lean"

# Copies the lane's inputs (dereferencing runfiles links) into a workspace that keeps the
# repository layout, so provenance paths in the translation are stable.
v1_copy_sources() {
  local ws="$1" f
  mkdir -p "$ws"
  for f in $(cd "$RUNFILES_ROOT" && { find lean examples/crossing_v1/spec examples/crossing_v1/scaffold/src verification/aeneas_v1 \( -type f -o -type l \) ! -name '*.olean'; echo examples/crossing_v1/composition/L2RDerived/crossing_v1.lean; }); do
    mkdir -p "$ws/$(dirname "$f")"
    cp -L "$RUNFILES_ROOT/$f" "$ws/$f"
  done
}

# v1_extract <workspace> <dest>: Charon + Aeneas on the harness, from the workspace root.
v1_extract() {
  local ws="$1" dest="$2"
  mkdir -p "$dest"
  (cd "$ws" && env CHARON_TOOLCHAIN_IS_IN_PATH=1 PATH="$TOOLS/rust/bin:/usr/bin:/bin" "$DYLIB_VAR=$TOOLS/rust/lib" \
      "$CHARON" rustc --preset=aeneas --dest-file "$dest/crossing_v1_core.llbc" -- \
      verification/aeneas_v1/crossing_v1_core.rs --crate-type=lib --edition=2024)
  (cd "$ws" && env "$DYLIB_VAR=$TOOLS/aeneas/libs" "$AENEAS" -backend lean "$dest/crossing_v1_core.llbc" -dest "$dest")
}

# v1_compile_all <workspace> <olean-dir> [translation]: every module of V1_MODULES, in order.
v1_compile_all() {
  local ws="$1" out="$2" translation="${3:-}" root file
  if [ -n "$translation" ]; then cp "$translation" "$ws/verification/aeneas_v1/proof/Extracted/CrossingV1Core.lean"; fi
  while read -r root file; do
    mkdir -p "$out/$(dirname "$file")"
    LEAN_PATH="$LEAN_LIBS:$out" "$LEAN" -R "$ws/$root" "$ws/$root/$file" -o "$out/${file%.lean}.olean" || return 1
  done <<< "$V1_MODULES"
}

v1_audit() {
  local ws="$1" out="$2" p="$1/verification/aeneas_v1/proof"
  LEAN_PATH="$LEAN_LIBS:$out" "$LEAN" -R "$p" "$p/Audit.lean"
}

# Re-checks every declaration of the lane's modules with leanchecker. Imported Aeneas and
# mathlib .oleans are trusted, not replayed (a full replay of mathlib does not fit a PR gate).
v1_kernel_check() {
  local out="$1" m
  for m in L2R.Async L2R.Async1 CrossingV1.Latch CrossingV1.Signal CrossingV1.Ports CrossingV1.Routes CrossingV1 \
           L2RDerived.crossing_v1 AeneasV1.Model Extracted.CrossingV1Core Refinement; do
    PATH="$TOOLS/lean/bin:/usr/bin:/bin" LEAN_PATH="$LEAN_LIBS:$out" "$TOOLS/lean/bin/leanchecker" "$m" || return 1
  done
}

# v1_run_mutants <workspace> <olean-dir> <evidence>: every line of mutants.tsv is
#   name <TAB> rust file <TAB> exact text <TAB> replacement <TAB> theorem
v1_run_mutants() {
  local ws="$1" out="$2" evidence="$3" count=0 name file from to theorem
  while IFS=$'\t' read -r name file from to theorem; do
    [ -z "$name" ] || [ "${name:0:1}" = "#" ] && continue
    count=$((count + 1))
    local mws="$TEST_TMPDIR/mutant-$name"
    rm -rf "$mws"; cp -R "$ws" "$mws"
    apply_edit "$from" "$to" "$mws/$file" || fail "mutant $name: pattern not found in $file"
    cmp -s "$ws/$file" "$mws/$file" && fail "mutant $name: the source did not change"
    v1_extract "$mws" "$mws/extracted" > "$evidence/mutant-$name-extract.log" 2>&1 || fail "mutant $name: extraction failed"
    local log="$evidence/mutant-$name.log"
    # Only the translation and the refinement are rebuilt; the model bridge does not depend on Rust.
    local mout="$mws/olean"; mkdir -p "$mout"; cp -R "$out"/. "$mout"/
    cp "$mws/extracted/CrossingV1Core.lean" "$mws/verification/aeneas_v1/proof/Extracted/CrossingV1Core.lean"
    local p="$mws/verification/aeneas_v1/proof"
    if ! LEAN_PATH="$LEAN_LIBS:$mout" "$LEAN" -R "$p" "$p/Extracted/CrossingV1Core.lean" -o "$mout/Extracted/CrossingV1Core.olean" > "$log" 2>&1; then
      fail "mutant $name: the mutated translation does not even compile (setup error): $(head -c 1500 "$log")"
    fi
    if LEAN_PATH="$LEAN_LIBS:$mout" "$LEAN" -R "$p" "$p/Refinement.lean" -o "$mout/Refinement.olean" >> "$log" 2>&1; then
      fail "mutant $name was NOT detected: the proof still checks"
    fi
    local range start end lines line
    range="$(theorem_range "$p/Refinement.lean" "$theorem")"
    [ -n "$range" ] || fail "mutant $name: theorem $theorem not found"
    read -r start end <<< "$range"
    lines="$(grep -oE 'Refinement\.lean:[0-9]+:[0-9]+: error' "$log" | sed -E 's/Refinement\.lean:([0-9]+):.*/\1/' | sort -un)"
    [ -n "$lines" ] || fail "mutant $name: rejected, but not by Refinement.lean (setup error?): $(head -c 2000 "$log")"
    for line in $lines; do
      if [ "$line" -lt "$start" ] || [ "$line" -gt "$end" ]; then
        fail "mutant $name: error at Refinement.lean:$line is outside $theorem ($start-$end)"
      fi
    done
    echo "mutant $name: rejected by $theorem (errors at lines $(echo $lines))" | tee -a "$evidence/mutants.txt"
  done < "$ws/verification/aeneas_v1/mutants.tsv"
  [ "$count" -gt 0 ] || fail "no mutants defined"
}

# v1_run_bypasses <workspace> <olean-dir> <evidence>: every line of bypasses.tsv is
#   name <TAB> proof file <TAB> exact text <TAB> replacement <TAB> gate
# and must be rejected by that gate: `proof` (does not compile), `audit` (compiles, but the audit or
# the statement comparison rejects it) or `kernel` (passes the audit, the kernel replay rejects).
v1_run_bypasses() {
  local ws="$1" out="$2" evidence="$3" count=0 name file from to gate
  while IFS=$'\t' read -r name file from to gate; do
    [ -z "$name" ] || [ "${name:0:1}" = "#" ] && continue
    count=$((count + 1))
    local bws="$TEST_TMPDIR/bypass-$name" log="$evidence/bypass-$name.log" bout="$TEST_TMPDIR/bypass-$name-olean"
    rm -rf "$bws" "$bout"; cp -R "$ws" "$bws"; mkdir -p "$bout"
    apply_edit "$from" "$to" "$bws/$file" || fail "bypass $name: pattern not found in $file"
    if ! v1_compile_all "$bws" "$bout" > "$log" 2>&1; then
      [ "$gate" = proof ] || fail "bypass $name: expected a later gate to reject it, but the proof did not compile"
      echo "bypass $name: rejected by proof checking" | tee -a "$evidence/bypasses.txt"
      continue
    fi
    [ "$gate" = audit ] || [ "$gate" = kernel ] || fail "bypass $name was NOT rejected: the proof still compiles"
    local audited=rejected
    if v1_audit "$bws" "$bout" >> "$log" 2>&1 && grep -q "AUDIT PASSED" "$log"; then
      grep -E "^(DEFINITION|CONSTRUCTOR|STATEMENT) " "$log" | cmp -s - "$bws/verification/aeneas_v1/expected-statements.txt" && audited=accepted
    fi
    if [ "$gate" = kernel ]; then
      [ "$audited" = accepted ] || fail "bypass $name: expected only the kernel replay to reject it, but the audit did"
      if v1_kernel_check "$bout" >> "$log" 2>&1; then
        fail "bypass $name was NOT rejected by the kernel replay"
      fi
      echo "bypass $name: passes the audit, rejected by the kernel replay ($(grep -m1 -oE '\(kernel\) .{0,80}' "$log" || echo 'leanchecker failed'))" | tee -a "$evidence/bypasses.txt"
      continue
    fi
    [ "$audited" = rejected ] || fail "bypass $name was NOT rejected by the audit"
    echo "bypass $name: rejected by the audit ($(grep -m1 -oE 'AUDIT FAILED: .{0,120}' "$log" || echo 'statement mismatch'))" | tee -a "$evidence/bypasses.txt"
  done < "$ws/verification/aeneas_v1/bypasses.tsv"
  [ "$count" -gt 0 ] || fail "no bypasses defined"
}
