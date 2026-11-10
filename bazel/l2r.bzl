"""Public Bazel rules for projects that use L2R.

    load("@l2r//bazel:l2r.bzl", "l2r_check_test")

    l2r_check_test(name = "scaffold_matches_approved_plan_test")

in the package that holds `architecture.toml` runs `l2r check` as a test: it
fails when a generated file differs from the approved plan, when the approval
record is stale (the model, the Lean inventory, the generator or a profile
changed since it was recorded), when an interrupted generation is pending, or
when an implementation file exceeds its budget.
"""

load("@rules_rust//rust:defs.bzl", "rust_binary")
load("@rules_shell//shell:sh_test.bzl", "sh_test")

def l2r_check_test(
        name,
        model = "architecture.toml",
        inventory = "lean-inventory.json",
        approval = "architecture.approval.json",
        scaffold = "scaffold",
        **kwargs):
    """Runs `l2r check` on a model and its generated scaffold.

    Args:
      name: name of the test target.
      model: the architecture model, a file of this package.
      inventory: the Lean inventory the model names.
      approval: the approval record `l2r approve` writes beside the model.
      scaffold: the model's `output_dir`, a sub-package of this one; its
        generated `BUILD.bazel` defines the `l2r_files` target used here.
      **kwargs: passed to the underlying `sh_test`.
    """
    l2r = Label("//crates/l2r_cli:l2r")
    script = Label("//bazel:check_scaffold.sh")

    # Until a plan has been approved and generated there is nothing to compare
    # with. The test then exists and fails with what to do next, instead of
    # leaving the project silently unchecked.
    if not native.glob([approval], allow_empty = True):
        sh_test(
            name = name,
            srcs = [script],
            args = ["--not-generated", native.package_name() + "/" + model],
            **kwargs
        )
        return

    package = native.package_name()
    scaffold_label = "//{}{}{}:l2r_files".format(package, "/" if package else "", scaffold)
    sh_test(
        name = name,
        srcs = [script],
        args = ["$(rootpath {})".format(l2r), "$(rootpath {})".format(model)],
        data = [model, inventory, approval, scaffold_label, l2r],
        **kwargs
    )

def l2r_conformance_traces(
        name,
        crate,
        src = "verification/traces.rs",
        scaffold = "scaffold",
        **kwargs):
    """The recorder of the derived conformance harness, as a binary.

    `l2r conformance <model> --rust > verification/traces.rs` derives a program
    that drives the generated `System` through operation sequences and prints
    what it observes; `l2r conformance <model> --lean` derives the Lean program
    that replays them on the scheduler model. The target exists once `src` does.

    Args:
      name: name of the binary target.
      crate: the model's `rust_crate`.
      src: the derived Rust program, a file of this package.
      scaffold: the model's `output_dir`, a sub-package of this one.
      **kwargs: passed to the underlying `rust_binary`.
    """
    if not native.glob([src], allow_empty = True):
        return
    package = native.package_name()

    # A derived file: its layout is the generator's, not rustfmt's.
    kwargs["tags"] = kwargs.get("tags", []) + ["norustfmt"]
    rust_binary(
        name = name,
        srcs = [src],
        edition = "2024",
        deps = ["//{}{}{}:{}".format(package, "/" if package else "", scaffold, crate)],
        **kwargs
    )
