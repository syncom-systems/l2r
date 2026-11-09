"""Public Bazel rules for projects that use L2R.

    load("@l2r//bazel:l2r.bzl", "l2r_check_test")

    l2r_check_test(name = "scaffold_matches_approved_plan_test")

in the package that holds `architecture.toml` runs `l2r check` as a test: it
fails when a generated file differs from the approved plan, when the approval
record is stale (the model, the Lean inventory, the generator or a profile
changed since it was recorded), when an interrupted generation is pending, or
when an implementation file exceeds its budget.
"""

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
