#!/usr/bin/env bash
# Regenerates the committed translation from the committed Rust sources:
#   bazel run --config=verify //verification/aeneas:update_translation
# Review the diff: a changed translation changes what the proofs are about.
set -euo pipefail
# Both `bazel run` and `bazel test` start in the runfiles root.
source verification/aeneas/toolchain.sh "$1"
grep -q '^status=available' "$1" || { echo "use --config=verify" >&2; exit 1; }
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
copy_sources "$work/ws"
extract "$work/ws" "$work/out"
cp "$work/out/CrossingCore.lean" "$BUILD_WORKSPACE_DIRECTORY/verification/aeneas/proof/Extracted/CrossingCore.lean"
echo "updated verification/aeneas/proof/Extracted/CrossingCore.lean"
