#!/usr/bin/env bash
# Scheduled dependency recheck: replays, through the Lean kernel, the declarations of the
# modules the proof lane imports (mathlib, Aeneas' Lean library, batteries, ...) and of the
# lane's own modules. The PR lane trusts the imported ones (refinement_test.sh, gate 4b); this
# job does not.
#
#   bazel test --config=verify //verification/aeneas:dependency_recheck
#
# Never part of `bazel test //...` or of the PR lane (tags: manual, dependency-recheck).
#
# How: `leanchecker --fresh <Root>` replays the whole import closure in ONE single-threaded
# process, which did not finish in 55 CPU-minutes. Instead the import closure of `Refinement` is
# computed and copied (hard-linked) into a private LEAN_PATH, and `leanchecker <Prefix>` replays,
# for each shard of modules, those modules' own declarations on top of their imports, in
# parallel inside one process. Together the shards cover every declaration of every closure
# module once. Prefix matching only sees the private LEAN_PATH, so only closure modules are
# replayed, not all of mathlib.
#
# Bounds (environment; pass with --test_env=NAME=value):
#   RECHECK_BUDGET_SECONDS  wall-clock budget for the replay phase (default 5400). When it runs
#                           out the run is reported "incomplete" and the test FAILS.
#   RECHECK_THREADS         LEAN_NUM_THREADS of each leanchecker process (default 2).
#   RECHECK_MAX_SHARD       packages with more modules than this are split by their second name
#                           component (default 200); smaller shards bound memory per process
#                           and give finer progress.
#   RECHECK_CORE            1 to also replay Init/Std/Lean/Lake, which ship with (and are
#                           compiled by) the trusted Lean binaries (default 0).
#   RECHECK_ROOTS           modules whose import closure is replayed (default: Refinement).
#
# Evidence: $TEST_UNDECLARED_OUTPUTS_DIR/dependency-recheck.json (+ modules.tsv, shards.tsv and
# one log per shard). status is "complete" (exit 0), "failed" (a shard was rejected or crashed:
# exit 1) or "incomplete" (budget exhausted before every shard finished: exit 124). Only
# "complete" passes.
set -uo pipefail

manifest="$1"
if ! grep -q '^status=available' "$manifest"; then
  echo "FAIL: verification toolchain unavailable: $(sed -n 's/^reason=//p' "$manifest")" >&2
  exit 1
fi
source verification/aeneas/toolchain.sh "$manifest"

budget="${RECHECK_BUDGET_SECONDS:-5400}"
threads="${RECHECK_THREADS:-2}"
max_shard="${RECHECK_MAX_SHARD:-200}"
core="${RECHECK_CORE:-0}"
roots="${RECHECK_ROOTS:-Refinement}"

evidence="${TEST_UNDECLARED_OUTPUTS_DIR:-$TEST_TMPDIR/evidence}"
mkdir -p "$evidence/shards"
cp "$manifest" "$evidence/toolchain-manifest.txt"
: > "$evidence/shards.tsv"
: > "$evidence/modules.tsv"
started=$(date +%s)
status=failed
reason="setup did not finish"

write_evidence() {
  EV_DIR="$evidence" EV_STATUS="$status" EV_REASON="$reason" EV_START="$started" EV_BUDGET="$budget" \
  EV_THREADS="$threads" EV_CORE="$core" EV_ROOTS="$roots" EV_MAX_SHARD="$max_shard" \
  EV_LAKE_MANIFEST="$TOOLS/aeneas/backends/lean/lake-manifest.json" EV_CHECKER="$TOOLS/lean/bin/leanchecker" \
  perl -MJSON::PP -e '
    sub slurp { local $/; open(my $f, "<", $_[0]) or return ""; <$f> }
    sub lines { [ grep { length } split /\n/, slurp($_[0]) ] }
    my $dir = $ENV{EV_DIR};
    my %man = map { /^([^=]+)=(.*)$/ ? ($1 => $2) : () } split /\n/, slurp("$dir/toolchain-manifest.txt");
    my $lake = eval { decode_json(slurp($ENV{EV_LAKE_MANIFEST})) } || {};
    my %rev = map { $_->{name} => $_->{rev} } @{ $lake->{packages} || [] };
    my @mods = @{ lines("$dir/modules.tsv") };
    my @shards = map { my @f = split /\t/; { prefix => $f[0], modules => $f[1] + 0, status => $f[2], seconds => $f[3] + 0, exit => $f[4] + 0 } } @{ lines("$dir/shards.tsv") };
    my $checked = 0; $checked += $_->{modules} for grep { $_->{status} eq "ok" } @shards;
    my $log = "";
    if (my ($bad) = grep { $_->{status} ne "ok" } @shards) {
      $log = slurp("$dir/shards/$bad->{prefix}.log");
      $log = substr($log, -4000) if length($log) > 4000;
    }
    my ($sum) = split " ", `shasum -a 256 "$ENV{EV_CHECKER}" 2>/dev/null`;
    my $out = {
      schema => 1, status => $ENV{EV_STATUS}, reason => $ENV{EV_REASON},
      wall_seconds => time - $ENV{EV_START}, budget_seconds => $ENV{EV_BUDGET} + 0,
      threads_per_process => $ENV{EV_THREADS} + 0, max_shard_modules => $ENV{EV_MAX_SHARD} + 0,
      core_included => $ENV{EV_CORE} eq "1" ? JSON::PP::true : JSON::PP::false,
      closure_roots => [ split " ", $ENV{EV_ROOTS} ],
      toolchain_manifest => \%man, lean_package_revisions => \%rev,
      leanchecker_sha256 => $sum,
      modules_in_closure => scalar(@mods), modules_checked => $checked,
      shards => \@shards, log_tail => $log,
    };
    open(my $o, ">", "$dir/dependency-recheck.json") or die "cannot write evidence";
    print $o JSON::PP->new->canonical->pretty->encode($out);
  '
}

finish() {
  trap - EXIT
  write_evidence || echo "warning: could not write the evidence file" >&2
  echo "dependency recheck: $status ($reason)"
  case "$status" in complete) exit 0;; incomplete) exit 124;; *) exit 1;; esac
}
# Any unexpected exit (set -u, a killed child) still leaves evidence saying "failed".
trap 'reason="script aborted: $reason"; finish' EXIT

# 1. Compile the lane's own modules (the committed translation; no re-extraction).
ws="$TEST_TMPDIR/ws"
copy_sources "$ws"
spec_out="$TEST_TMPDIR/olean-spec"
proof_out="$TEST_TMPDIR/olean-proof"
if ! { compile_spec "$ws" "$spec_out" && compile_proof "$ws" "$spec_out" "$proof_out"; } > "$evidence/compile.log" 2>&1; then
  tail -20 "$evidence/compile.log" >&2
  reason="the lane's own modules do not compile"; finish
fi

# 2. The import closure: module <TAB> olean path.
if ! PATH="$TOOLS/lean/bin:/usr/bin:/bin" LEAN_PATH="$LEAN_LIBS:$spec_out:$proof_out" "$LEAN" --run "$ws/verification/aeneas/DependencyClosure.lean" $roots \
    > "$evidence/modules.tsv" 2> "$evidence/closure.log"; then
  cat "$evidence/closure.log" >&2
  reason="could not compute the import closure"; finish
fi

# 3. A private LEAN_PATH holding the closure modules outside the Lean sysroot: hard links, or
#    copies across file systems (~1 GB). Symbolic links are not followed by the module search.
farm="$TEST_TMPDIR/closure-lib"
sysroot="$(cd "$TOOLS/lean/lib/lean" && pwd -P)"
rm -rf "$farm"; mkdir -p "$farm"
while IFS=$'\t' read -r mod path; do
  case "$(cd "$(dirname "$path")" && pwd -P)/" in "$sysroot"/*) continue;; esac
  rel="${mod//.//}"
  mkdir -p "$farm/$(dirname "$rel")"
  for f in "${path%.olean}".olean*; do
    ln "$f" "$farm/$(dirname "$rel")/$(basename "$f")" 2> /dev/null \
      || cp -L "$f" "$farm/$(dirname "$rel")/$(basename "$f")"
  done
done < "$evidence/modules.tsv"

# 4. Shards: one per top-level package, or per second name component for packages with more
#    than max_shard modules. A prefix covers its own module and every module below it.
cut -f1 "$evidence/modules.tsv" \
  | { if [ "$core" = 1 ]; then cat; else grep -v -E '^(Init|Std|Lean|Lake)(\.|$)' || true; fi; } \
  | awk -v max="$max_shard" '
      { mod[NR] = $0; split($0, p, "."); top[p[1]]++; if (index($0, ".") == 0) bare[p[1]] = 1 }
      END { for (i = 1; i <= NR; i++) {
              n = split(mod[i], p, ".")
              key = (top[p[1]] > max && n > 1 && !(p[1] in bare)) ? p[1] "." p[2] : p[1]
              count[key]++ }
            for (k in count) print k "\t" count[k] }' \
  | sort > "$evidence/shard-plan.tsv"
total="$(wc -l < "$evidence/shard-plan.tsv" | tr -d ' ')"
if [ "$total" -eq 0 ]; then reason="no shards to check"; finish; fi
echo "closure: $(wc -l < "$evidence/modules.tsv" | tr -d ' ') modules, $total shards, budget ${budget}s, ${threads} threads"

# 5. Replay, smallest shards first so that a budget overrun loses the least.
status=incomplete
reason="budget exhausted before every shard finished"
deadline=$(( $(date +%s) + budget ))
done_count=0
failed=0
while IFS=$'\t' read -r prefix count; do
  t0=$(date +%s)
  [ "$t0" -lt "$deadline" ] || break
  log="$evidence/shards/$prefix.log"
  PATH="$TOOLS/lean/bin:/usr/bin:/bin" LEAN_PATH="$farm" LEAN_NUM_THREADS="$threads" \
    "$TOOLS/lean/bin/leanchecker" "$prefix" > "$log" 2>&1 &
  pid=$!
  timed_out=0
  while kill -0 "$pid" 2> /dev/null; do
    if [ "$(date +%s)" -ge "$deadline" ]; then
      timed_out=1
      kill -TERM "$pid" 2> /dev/null
      sleep 5
      kill -KILL "$pid" 2> /dev/null
      break
    fi
    sleep 2
  done
  wait "$pid" 2> /dev/null
  rc=$?
  secs=$(( $(date +%s) - t0 ))
  if [ "$timed_out" = 1 ]; then st=timeout; rc=124
  elif [ "$rc" = 0 ]; then st=ok; done_count=$((done_count + 1))
  else st=failed; failed=1; fi
  printf '%s\t%s\t%s\t%s\t%s\n' "$prefix" "$count" "$st" "$secs" "$rc" >> "$evidence/shards.tsv"
  echo "shard $prefix ($count modules): $st in ${secs}s"
  [ "$st" != timeout ] || break
done < <(sort -t$'\t' -k2,2n "$evidence/shard-plan.tsv")

if [ "$failed" = 1 ]; then
  status=failed; reason="leanchecker rejected a shard or crashed (see shards/*.log)"
elif [ "$done_count" = "$total" ]; then
  status=complete; reason="every shard replayed"
fi
finish
