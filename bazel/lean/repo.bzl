"""The pinned Lean toolchain on its own: `lean` and `lake` at the version every
Lean file in this repository is checked with.

The same release archive, verified by the same sha256, as the proof lane uses
(//bazel/verification:repo.bzl is the single place the pin is recorded). This
repository contains nothing else, so that a project using L2R can run
`l2r inventory` and replay its conformance traces with a reproducible Lean
without fetching the proof lane's ~8 GB:

    export PATH="$(bazel run @l2r//tools/lean:bin_path)":$PATH

Nothing in the default build refers to this repository; it is fetched the
first time //tools/lean is built.
"""

load("//bazel/verification:repo.bzl", "LEAN_VERSION", "PINS", "platform_key")

def _lean_toolchain_impl(rctx):
    platform = platform_key(rctx)
    if platform == None:
        rctx.file("bin_path.txt", "")
        rctx.file("reason.txt", "no pinned Lean for {} {}; install Lean {} (https://lean-lang.org/install)\n".format(rctx.os.name, rctx.os.arch, LEAN_VERSION))
    else:
        url, sha, prefix = PINS[platform]["lean"]
        rctx.download_and_extract(url, output = "lean", sha256 = sha, stripPrefix = prefix)

        # An absolute path into this repository: `lake` is run from the user's
        # shell, outside any Bazel action.
        rctx.file("bin_path.txt", str(rctx.path("lean/bin")) + "\n")
        rctx.file("reason.txt", "")
    rctx.file("BUILD.bazel", 'exports_files(["bin_path.txt", "reason.txt"])\n')

lean_toolchain = repository_rule(
    implementation = _lean_toolchain_impl,
    doc = "The pinned Lean release (lean, lake), verified by sha256.",
)
