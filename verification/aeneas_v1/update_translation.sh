#!/usr/bin/env bash
# Regenerates the committed translation from the committed generated Rust:
#   bazel run --config=verify //verification/aeneas_v1:update_translation
# Review the diff: a changed translation changes what the proofs are about.
set -euo pipefail
# Both `bazel run` and `bazel test` start in the runfiles root.
source verification/aeneas/toolchain.sh "$1"
source verification/aeneas_v1/lane.sh
grep -q '^status=available' "$1" || { echo "use --config=verify" >&2; exit 1; }
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
v1_copy_sources "$work/ws"
v1_extract "$work/ws" "$work/out"
cp "$work/out/CrossingV1Core.lean" "$BUILD_WORKSPACE_DIRECTORY/verification/aeneas_v1/proof/Extracted/CrossingV1Core.lean"
echo "updated verification/aeneas_v1/proof/Extracted/CrossingV1Core.lean"
