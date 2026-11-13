"""Rust crates from Bazel targets: the attributes of the `rules_rust` rules
(`crate_name`, `crate_root`, `deps`, `proc_macro_deps`) and how targets of one
crate are merged.
"""

from __future__ import annotations

from pathlib import Path

from .. import bazel
from ..model import Unit
from .crates import Crate, crate_root

KINDS = ["rust_library", "rust_binary", "rust_proc_macro"]


def from_target(repo: Path, t: bazel.Target) -> Crate | None:
    srcs = t.sources(".rs")
    if not srcs:
        return None
    files = [bazel.label_path(repo, s) for s in srcs]
    root = t.values.get("crate_root")
    name = (t.values.get("crate_name") or t.label.rsplit(":", 1)[-1]).replace("-", "_")
    unit = Unit(id=name, kind="rust_crate", package=t.package, declared_at=t.declared_at,
                labels=[t.label], testonly=t.testonly, visibility=t.labels("visibility"))
    if t.configurable:
        unit.not_measured.append("select() in its attributes; the union of the branches is measured")
    return Crate(name=name, root=crate_root(files, root and bazel.label_path(repo, root)),
                 files=files, unit=unit, dep_ids=t.labels("deps") + t.labels("proc_macro_deps"))


def merge_variant(crates: dict[tuple[str, Path], Crate], crate: Crate) -> None:
    """Targets built from the same root under the same name are one crate (e.g. a feature variant)."""
    same = crates.setdefault((crate.name, crate.root), crate)
    if same is not crate:
        same.unit.labels += crate.unit.labels
        same.dep_ids = sorted(set(same.dep_ids) | set(crate.dep_ids))
        same.unit.testonly = same.unit.testonly and crate.unit.testonly
        same.unit.visibility = sorted(set(same.unit.visibility) | set(crate.unit.visibility))


def from_bazel(repo: Path, targets: list[bazel.Target]) -> list[Crate]:
    crates: dict[tuple[str, Path], Crate] = {}
    for t in targets:
        crate = from_target(repo, t)
        if crate:
            merge_variant(crates, crate)
    return list(crates.values())
