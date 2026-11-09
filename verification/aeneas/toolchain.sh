# Shared functions for the proof lane. Sourced with the toolchain manifest path.
# shellcheck shell=bash

_manifest="$1"
TOOLS="$(cd "$(dirname "$(realpath "$_manifest")")" && pwd -P)"
DYLIB_VAR="$(sed -n 's/^dylib_var=//p' "$_manifest")"
LEAN="$TOOLS/lean/bin/lean"
CHARON="$TOOLS/aeneas/charon"
AENEAS="$TOOLS/aeneas/aeneas"
_lib="$TOOLS/aeneas/backends/lean"
LEAN_LIBS="$_lib/.lake/build/lib/lean"
for _p in "$_lib"/.lake/packages/*; do LEAN_LIBS="$LEAN_LIBS:$_p/.lake/build/lib/lean"; done

RUST_SOURCES="examples/crossing/scaffold/src/crossing/request_latch/mod.rs
examples/crossing/scaffold/src/crossing/request_latch/imp.rs
examples/crossing/scaffold/src/crossing/signal/mod.rs
examples/crossing/scaffold/src/crossing/signal/imp.rs
examples/crossing/scaffold/src/crossing/routes.rs"

# Copies the lane's inputs (dereferencing runfiles links) into a workspace that
# keeps the repository layout, so provenance paths in the translation are stable.
copy_sources() {
  local ws="$1"
  mkdir -p "$ws"
  local f
  for f in $RUST_SOURCES examples/crossing/lean-inventory.json tools/lean_inventory/Main.lean \
           $(cd "$RUNFILES_ROOT" && find examples/crossing/spec verification/aeneas -type f -o -type l); do
    mkdir -p "$ws/$(dirname "$f")"
    cp -L "$RUNFILES_ROOT/$f" "$ws/$f"
  done
}

# extract <workspace> <dest>: Charon + Aeneas on the harness, from the workspace root.
extract() {
  local ws="$1" dest="$2"
  mkdir -p "$dest"
  (cd "$ws" && env CHARON_TOOLCHAIN_IS_IN_PATH=1 PATH="$TOOLS/rust/bin:/usr/bin:/bin" "$DYLIB_VAR=$TOOLS/rust/lib" \
      "$CHARON" rustc --preset=aeneas --dest-file "$dest/crossing_core.llbc" -- \
      verification/aeneas/crossing_core.rs --crate-type=lib --edition=2024)
  (cd "$ws" && env "$DYLIB_VAR=$TOOLS/aeneas/libs" "$AENEAS" -backend lean "$dest/crossing_core.llbc" -dest "$dest")
}

# lean_compile <root> <file> <olean-out> <extra-lean-path>
lean_compile() {
  local root="$1" file="$2" out="$3" extra="$4"
  mkdir -p "$(dirname "$out")"
  LEAN_PATH="$LEAN_LIBS:$extra" "$LEAN" -R "$root" "$file" -o "$out"
}

# kernel_check <spec-oleans> <proof-oleans>: re-checks every declaration of the
# spec and proof modules with leanchecker. Imported Aeneas and mathlib .oleans are
# trusted, not replayed (a full replay of mathlib does not fit a PR gate).
kernel_check() {
  local spec="$1" out="$2" m
  for m in Crossing.Latch Crossing.Signal Crossing.Routes Crossing Extracted.CrossingCore Refinement; do
    PATH="$TOOLS/lean/bin:/usr/bin:/bin" LEAN_PATH="$LEAN_LIBS:$spec:$out" "$TOOLS/lean/bin/leanchecker" "$m" || return 1
  done
}

compile_spec() {
  local ws="$1" out="$2" m
  for m in Crossing/Latch Crossing/Signal Crossing/Routes Crossing; do
    lean_compile "$ws/examples/crossing/spec" "$ws/examples/crossing/spec/$m.lean" "$out/$m.olean" "$out"
  done
}

# compile_proof <workspace> <spec-oleans> <out> [translation]: the translation
# (default: the committed one), then Refinement.lean.
compile_proof() {
  local ws="$1" spec="$2" out="$3" translation="${4:-}"
  local p="$ws/verification/aeneas/proof"
  if [ -n "$translation" ]; then cp "$translation" "$p/Extracted/CrossingCore.lean"; fi
  lean_compile "$p" "$p/Extracted/CrossingCore.lean" "$out/Extracted/CrossingCore.olean" "$spec:$out"
  lean_compile "$p" "$p/Refinement.lean" "$out/Refinement.olean" "$spec:$out"
}

audit_proof() {
  local ws="$1" spec="$2" out="$3" p="$ws/verification/aeneas/proof"
  LEAN_PATH="$LEAN_LIBS:$spec:$out" "$LEAN" -R "$p" "$p/Audit.lean"
}

# apply_edit <exact text> <replacement> <file>: literal replacement of the first
# occurrence; `\n` in either text stands for a newline.
apply_edit() {
  perl -0pi -e 'BEGIN { ($from, $to) = (shift, shift); s/\\n/\n/g for $from, $to }
                s/\Q$from\E/$to/ or die "pattern not found\n"' "$1" "$2" "$3"
}

# Line range of `theorem <name>` in Refinement.lean: its line up to the line
# before the next top-level declaration or section end.
theorem_range() {
  local file="$1" name="$2"
  awk -v n="$name" '
    $0 ~ "^theorem " n "( |$)" { start = NR; next }
    start && !end && /^(theorem|def|end|\/--|\/-!) / { end = NR - 1 }
    END { if (!end) end = NR; if (start) print start, end }' "$file"
}

# run_mutants <workspace> <spec-oleans> <evidence>: every line of mutants.tsv is
#   name <TAB> rust file <TAB> exact text <TAB> replacement <TAB> theorem
run_mutants() {
  local ws="$1" spec="$2" evidence="$3" count=0
  local name file from to theorem
  while IFS=$'\t' read -r name file from to theorem; do
    [ -z "$name" ] || [ "${name:0:1}" = "#" ] && continue
    count=$((count + 1))
    local mws="$TEST_TMPDIR/mutant-$name"
    rm -rf "$mws"; cp -R "$ws" "$mws"
    # Literal (not regex) replacement of the first occurrence; portable across bash versions.
    apply_edit "$from" "$to" "$mws/$file" || fail "mutant $name: pattern not found in $file"
    cmp -s "$ws/$file" "$mws/$file" && fail "mutant $name: the source did not change"
    extract "$mws" "$mws/extracted" > "$evidence/mutant-$name-extract.log" 2>&1 || fail "mutant $name: extraction failed"
    local log="$evidence/mutant-$name.log"
    if compile_proof "$mws" "$spec" "$mws/olean" "$mws/extracted/CrossingCore.lean" > "$log" 2>&1; then
      fail "mutant $name was NOT detected: the proof still checks"
    fi
    local range start end lines line
    range="$(theorem_range "$mws/verification/aeneas/proof/Refinement.lean" "$theorem")"
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
  done < "$ws/verification/aeneas/mutants.tsv"
  [ "$count" -gt 0 ] || fail "no mutants defined"
}

RUNFILES_ROOT="$PWD"

sha256() {
  if command -v sha256sum > /dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

# run_bypasses <workspace> <spec-oleans> <evidence>: every line of bypasses.tsv is
#   name <TAB> proof file <TAB> exact text <TAB> replacement <TAB> gate
# and must be rejected by that gate: `proof` (does not compile) or `audit`
# (compiles, but the audit or the statement comparison rejects it).
run_bypasses() {
  local ws="$1" spec="$2" evidence="$3" count=0
  local name file from to gate
  while IFS=$'\t' read -r name file from to gate; do
    [ -z "$name" ] || [ "${name:0:1}" = "#" ] && continue
    count=$((count + 1))
    local bws="$TEST_TMPDIR/bypass-$name" log="$evidence/bypass-$name.log"
    rm -rf "$bws"; cp -R "$ws" "$bws"
    apply_edit "$from" "$to" "$bws/$file" || fail "bypass $name: pattern not found in $file"
    if ! compile_proof "$bws" "$spec" "$bws/olean" > "$log" 2>&1; then
      [ "$gate" = proof ] || fail "bypass $name: expected the audit to reject it, but the proof did not compile"
      echo "bypass $name: rejected by proof checking" | tee -a "$evidence/bypasses.txt"
      continue
    fi
    [ "$gate" = audit ] || [ "$gate" = kernel ] || fail "bypass $name was NOT rejected: the proof still compiles"
    local audited=rejected
    if audit_proof "$bws" "$spec" "$bws/olean" >> "$log" 2>&1 && grep -q "AUDIT PASSED" "$log"; then
      grep -E "^(DEFINITION|STATEMENT) " "$log" | cmp -s - "$bws/verification/aeneas/expected-statements.txt" && audited=accepted
    fi
    if [ "$gate" = kernel ]; then
      # Passes the audit by construction; only the kernel replay can reject it.
      [ "$audited" = accepted ] || fail "bypass $name: expected only the kernel replay to reject it, but the audit did"
      if kernel_check "$spec" "$bws/olean" >> "$log" 2>&1; then
        fail "bypass $name was NOT rejected by the kernel replay"
      fi
      echo "bypass $name: passes the audit, rejected by the kernel replay ($(grep -m1 -oE '\(kernel\) .{0,80}' "$log" || echo 'leanchecker failed'))" | tee -a "$evidence/bypasses.txt"
      continue
    fi
    [ "$audited" = rejected ] || fail "bypass $name was NOT rejected by the audit"
    echo "bypass $name: rejected by the audit ($(grep -m1 -oE 'AUDIT FAILED: .{0,120}' "$log" || echo 'statement mismatch'))" | tee -a "$evidence/bypasses.txt"
  done < "$ws/verification/aeneas/bypasses.tsv"
  [ "$count" -gt 0 ] || fail "no bypasses defined"
}
