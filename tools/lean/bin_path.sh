#!/usr/bin/env bash
# Prints the directory holding the pinned `lean` and `lake`.
#   bin_path.sh <bin_path.txt> <reason.txt>
set -euo pipefail
dir="$(cat "$1")"
if [ -z "$dir" ]; then
  cat "$2" >&2
  exit 1
fi
[ -x "$dir/lake" ] || { echo "the pinned Lean is missing from $dir (run \`bazel fetch\` again)" >&2; exit 1; }
echo "$dir"
