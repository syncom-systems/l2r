"""A Rust crate as the build declares it, whichever build system that is
(`bazel_rules`, `cargo`). Each declared dependency lands in one bucket of its
unit: a crate of this workspace, an external crate, or unresolved.
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

from ..model import DECLARED, Dependency, NoMeasurement, Unit

ROOT_FILES = ("lib.rs", "main.rs")


@dataclass
class Crate:
    name: str           # the crate name, as code refers to it
    root: Path          # the crate root file
    files: list[Path]
    unit: Unit          # its id is unique among crates, and its module ids start with it
    dep_ids: list[str] = field(default_factory=list)       # labels or names, as declared
    declared: dict[str, str] = field(default_factory=dict)  # dep crate id -> the label as declared
    dep_at: dict[str, str] = field(default_factory=dict)    # declared label -> file:line, when known

    @property
    def key(self) -> str:
        return self.unit.id

    @property
    def deps(self) -> list[str]:
        """The crates of this workspace it declares, by id."""
        return self.unit.workspace_deps

    def add_dependency(self, dep: str, key: str | None) -> None:
        """Record declared dependency `dep`, which is crate `key` of this workspace or None."""
        if key:
            self.declared.setdefault(key, dep)
            self.unit.workspace_deps.append(key)
        elif is_external(dep):
            self.unit.external_deps.append(dep)
        else:
            self.unit.unresolved_deps.append(dep)

    def declared_dependency(self, dep: str) -> Dependency:
        label = self.declared[dep]
        at = self.dep_at.get(label, self.unit.declared_at)
        return Dependency(DECLARED, self.key, dep, f"{at}: {self.unit.labels[0]} deps {label}")


def is_external(dep: str) -> bool:
    """A label of another repository (`@crates//:serde`), or a Cargo name that is not a crate here."""
    return dep.startswith("@") or not any(ch in dep for ch in "/:")


def crate_root(files: list[Path], declared: Path | None) -> Path:
    if declared:
        return declared
    roots = [f for f in files if f.name in ROOT_FILES]
    if roots:
        return roots[0]
    if len(files) == 1:
        return files[0]
    raise NoMeasurement(f"no crate root among {files[:3]}")


def unique_keys(crates: list[Crate]) -> None:
    """Binaries in different packages may share a crate name; their ids must not."""
    count: dict[str, int] = defaultdict(int)
    for crate in crates:
        count[crate.name] += 1
    for crate in crates:
        if count[crate.name] > 1:
            crate.unit.id = f"{crate.name}__{crate.unit.package.replace('/', '_')}"


def resolve_deps(crates: list[Crate]) -> None:
    by_id = {label: c.key for c in crates for label in c.unit.labels} | {c.name: c.key for c in crates}
    for crate in crates:
        for dep in crate.dep_ids:
            crate.add_dependency(dep, by_id.get(dep))
        crate.unit.workspace_deps = sorted(set(crate.deps) - {crate.key})
