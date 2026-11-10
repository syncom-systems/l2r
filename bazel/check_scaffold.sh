#!/usr/bin/env bash
# Runs `l2r check` on a real copy of a project. Bazel runfiles are symlinks, and
# L2R deliberately refuses to read or write through symlinks, so the model's
# directory is first copied (dereferencing links) into the test's scratch
# directory. Used by `l2r_check_test` (//bazel:l2r.bzl).
set -euo pipefail
if [ "$1" = "--not-generated" ]; then
  echo "FAIL: $2 has no approval record yet, so there is no approved plan to check against." >&2
  echo "Run:  l2r preview $2" >&2
  echo "      l2r approve $2 --digest <digest from preview> --by <your name>" >&2
  echo "      l2r generate $2" >&2
  exit 1
fi
l2r="$PWD/$1"
model="$2"
work="$TEST_TMPDIR/project"
rm -rf "$work"
mkdir -p "$work"
cp -RL "$(dirname "$model")/." "$work/"
"$l2r" check "$work/$(basename "$model")"
