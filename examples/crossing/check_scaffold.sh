#!/usr/bin/env bash
# Runs `l2r check` on a real copy of the example. Bazel runfiles are symlinks, and
# L2R deliberately refuses to read or write through symlinks, so the example is
# first copied (dereferencing links) into the test's scratch directory.
set -euo pipefail
l2r="$PWD/$1"
work="$TEST_TMPDIR/crossing"
mkdir -p "$work"
cp -RL examples/crossing/architecture.toml examples/crossing/lean-inventory.json \
  examples/crossing/architecture.approval.json examples/crossing/scaffold "$work/"
"$l2r" check "$work/architecture.toml"
