"""Pinned verification toolchain: Lean, Charon, Aeneas and the Aeneas Lean library.

Every artifact comes from a public release and is verified by sha256 before use:

  * Lean 4 (GitHub release of leanprover/lean4);
  * Aeneas + Charon (one nightly release of AeneasVerif/aeneas, which also pins
    the Lean version and the mathlib revision its Lean library is built against);
  * the Rust nightly Charon's driver links against (rustc, rust-std, rustc-dev
    from static.rust-lang.org, hashes taken from that day's channel manifest).

mathlib's compiled libraries are fetched by mathlib's own `lake exe cache get` at
the revision Aeneas's lake-manifest pins. That step uses the network; its
output is not pinned by a hash recorded here (see docs/verification.md).

Nothing in the default build refers to this repository: the proof test selects
it only under `--config=verify` (see //bazel/verification:enabled), so a plain
`bazel test //...` never fetches the ~8 GB it needs.
"""

AENEAS_RELEASE = "nightly-2026.11.02-ab34bd2"
LEAN_VERSION = "4.35.0"
RUST_NIGHTLY = "2026-09-17"

_RUST = "https://static.rust-lang.org/dist/" + RUST_NIGHTLY + "/"
_AENEAS = "https://github.com/AeneasVerif/aeneas/releases/download/" + AENEAS_RELEASE + "/"
_LEAN = "https://github.com/leanprover/lean4/releases/download/v" + LEAN_VERSION + "/"

PINS = {
    "darwin_aarch64": {
        "lean": (_LEAN + "lean-4.35.0-darwin_aarch64.tar.zst", "c0c6f34f35ba1dcc5c4a5fd106fefab9eb5e8469712624dd2c868d51b63177c2", "lean-4.35.0-darwin_aarch64"),
        "aeneas": (_AENEAS + "aeneas-macos-aarch64.tar.gz", "1f6db91ae0ec8fe2174720b6b1a7a2dc9beda4f03c648a4765e7e0379294aa8f", ""),
        "rust_triple": "aarch64-apple-darwin",
        "rust": {
            "rustc": "a25f3366f4dd0b0c810ee1a98a699f6b30734e13165a5c57e6fbecf7c841f315",
            "rust-std": "d9fce2d60a1ad93ff4825f09e8227a1250b4e4c1271ed9096560ff62579ba7dd",
            "rustc-dev": "0b86529128ff099d491829c4f3483f385d9228f64593d389c0413093cab1164c",
        },
        "dylib_var": "DYLD_LIBRARY_PATH",
    },
    "linux_x86_64": {
        "lean": (_LEAN + "lean-4.35.0-linux.tar.zst", "7289597a44543246c9227b8cae501767e095ce17f6d448b95fc4a8dc97268f6d", "lean-4.35.0-linux"),
        "aeneas": (_AENEAS + "aeneas-linux-x86_64.tar.gz", "f8ea7c69036915c7330816b7f5a679efbe443b736e9058c03e91067c66ddade4", ""),
        "rust_triple": "x86_64-unknown-linux-gnu",
        "rust": {
            "rustc": "6c4b66572411880b75f845892414e9b0ea04dfbbc50e0ce5394bd162c3e0dda6",
            "rust-std": "ce0462a1f0c3d6bf22fb952918a9840f1e85e6c1c3edd1d7840a90ab0a08dd8c",
            "rustc-dev": "53e753dcf3977aa70779320f6dc3819b94f50a0529ac54725cb2e496a6b7fccb",
        },
        "dylib_var": "LD_LIBRARY_PATH",
    },
}

def _platform(rctx):
    os = rctx.os.name.lower()
    arch = rctx.os.arch.lower()
    if os.startswith("mac") and arch in ("aarch64", "arm64"):
        return "darwin_aarch64"
    if os.startswith("linux") and arch in ("amd64", "x86_64"):
        return "linux_x86_64"
    return None

def _stub(rctx, reason):
    rctx.file("manifest.txt", "status=unavailable\nreason=" + reason + "\n")
    rctx.file("BUILD.bazel", 'exports_files(["manifest.txt"])\n')

def _run(rctx, args, **kwargs):
    result = rctx.execute(args, **kwargs)
    if result.return_code != 0:
        fail("{} failed ({}):\n{}\n{}".format(" ".join([str(a) for a in args]), result.return_code, result.stdout[-4000:], result.stderr[-4000:]))
    return result

def _rust_sysroot(rctx, pins):
    triple = pins["rust_triple"]
    for component, sha in sorted(pins["rust"].items()):
        name = "{}-nightly-{}".format(component, triple)
        rctx.download_and_extract(_RUST + name + ".tar.xz", output = "dl/" + component, sha256 = sha, stripPrefix = name)
        listing = rctx.read("dl/{}/components".format(component)).strip().split("\n")
        for part in listing:
            _run(rctx, ["sh", "-c", 'mkdir -p rust && cp -R "dl/{}/{}/." rust/'.format(component, part.strip())])
    _run(rctx, ["rm", "-rf", "dl"])

def _aeneas_library(rctx):
    """Fetches mathlib's compiled libraries at the pinned revision and checks the
    prebuilt Aeneas library against them; then prunes everything but libraries."""
    root = str(rctx.path("."))
    env = {
        "PATH": root + "/lean/bin:/usr/bin:/bin",
        "HOME": root + "/home",
        "XDG_CACHE_HOME": root + "/home/cache",
    }
    lib = "aeneas/backends/lean"
    _run(rctx, ["lake", "exe", "cache", "get"], working_directory = lib, environment = env, timeout = 3600)
    _run(rctx, ["lake", "build", "Aeneas"], working_directory = lib, environment = env, timeout = 3600)
    _run(rctx, ["sh", "-c", """
set -e
cd aeneas/backends/lean/.lake/packages
for p in *; do
  if [ ! -d "$p/.lake/build/lib" ]; then
    rm -rf "$p"  # a build-time tool (e.g. the cache client), not a library
    continue
  fi
  find "$p" -mindepth 1 -maxdepth 1 ! -name .lake -exec rm -rf {} +
  find "$p/.lake" -mindepth 1 -maxdepth 1 ! -name build -exec rm -rf {} +
  find "$p/.lake/build" -mindepth 1 -maxdepth 1 ! -name lib -exec rm -rf {} +
done
cd "$OLDPWD"
rm -rf home
"""])

def _verification_tools_impl(rctx):
    platform = _platform(rctx)
    if platform == None:
        _stub(rctx, "no pinned verification toolchain for {} {}".format(rctx.os.name, rctx.os.arch))
        return
    pins = PINS[platform]
    url, sha, prefix = pins["lean"]
    rctx.download_and_extract(url, output = "lean", sha256 = sha, stripPrefix = prefix)
    url, sha, prefix = pins["aeneas"]
    rctx.download_and_extract(url, output = "aeneas", sha256 = sha, stripPrefix = prefix)
    _rust_sysroot(rctx, pins)
    _aeneas_library(rctx)
    toolchain = rctx.read("aeneas/rust-toolchain")
    if ('channel = "nightly-' + RUST_NIGHTLY + '"') not in toolchain:
        fail("Aeneas {} expects a different Rust nightly than {}:\n{}".format(AENEAS_RELEASE, RUST_NIGHTLY, toolchain))
    lean_toolchain = rctx.read("aeneas/backends/lean/lean-toolchain").strip()
    if lean_toolchain != "leanprover/lean4:v" + LEAN_VERSION:
        fail("Aeneas {} expects {}, but Lean {} is pinned".format(AENEAS_RELEASE, lean_toolchain, LEAN_VERSION))
    manifest = [
        "status=available",
        "platform=" + platform,
        "aeneas_release=" + AENEAS_RELEASE,
        "aeneas_sha256=" + pins["aeneas"][1],
        "lean_version=" + LEAN_VERSION,
        "lean_sha256=" + pins["lean"][1],
        "rust_nightly=" + RUST_NIGHTLY,
    ] + ["rust_{}_sha256={}".format(c, s) for c, s in sorted(pins["rust"].items())] + [
        "dylib_var=" + pins["dylib_var"],
    ]
    rctx.file("manifest.txt", "\n".join(manifest) + "\n")
    rctx.file("BUILD.bazel", 'exports_files(["manifest.txt"])\n')

verification_tools = repository_rule(
    implementation = _verification_tools_impl,
    doc = "Pinned Lean, Charon, Aeneas and Aeneas Lean library for the proof lane.",
)
