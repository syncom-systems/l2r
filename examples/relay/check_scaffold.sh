#!/usr/bin/env bash
# Runs `l2r check` on a real copy of the example. Bazel runfiles are symlinks, and
# L2R deliberately refuses to read or write through symlinks, so the example is
# first copied (dereferencing links) into the test's scratch directory.
set -euo pipefail
l2r="$PWD/$1"
work="$TEST_TMPDIR/relay"
mkdir -p "$work"
cp -RL examples/relay/architecture.toml examples/relay/lean-inventory.json \
  examples/relay/architecture.approval.json examples/relay/scaffold "$work/"
"$l2r" check "$work/architecture.toml"
