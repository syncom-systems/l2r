"""L2R Rust macros: the single place where the workspace lint policy lives."""

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")

# unsafe is denied everywhere; every rustc warning is an error.
L2R_RUSTC_FLAGS = [
    "-Dunsafe_code",
    "-Dwarnings",
    "-Dmissing_docs",
]

def _with_policy(kwargs):
    kwargs["edition"] = "2024"
    kwargs["rustc_flags"] = L2R_RUSTC_FLAGS + kwargs.pop("rustc_flags", [])
    return kwargs

def l2r_rust_library(name, **kwargs):
    rust_library(name = name, **_with_policy(kwargs))

def l2r_rust_binary(name, **kwargs):
    rust_binary(name = name, **_with_policy(kwargs))

def l2r_rust_test(name, **kwargs):
    rust_test(name = name, **_with_policy(kwargs))
