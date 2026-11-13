"""A Rust crate as the build declares it, whichever build system that is
(`bazel_rules`, `cargo`). Each declared dependency lands in one bucket: a
crate of this workspace, an external crate, or unresolved.
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

from ..graph import excluded
from ..model import DECLARED, Dependency, NoMeasurement, Unit

ROOT_FILES = ("lib.rs", "main.rs")


@dataclass
class Crate:
    key: str            # unique among crates; its module ids start with it
    name: str           # the crate name, as code refers to it
    root: Path          # the crate root file
    files: list[Path]
    unit: Unit
    dep_ids: list[str] = field(default_factory=list)       # labels or names, as declared
    deps: list[str] = field(default_factory=list)          # keys of crates in this workspace
    declared: dict[str, str] = field(default_factory=dict)  # dep key -> the label as declared
    dep_at: dict[str, str] = field(default_factory=dict)    # declared label -> file:line, when known

    def add_dependency(self, dep: str, key: str | None) -> None:
        """Record declared dependency `dep`, which is crate `key` of this workspace or None."""
        if key:
            self.declared.setdefault(key, dep)
            self.deps.append(key)
        elif is_external(dep):
            self.unit.external_deps.append(dep)
        else:
            self.unit.unresolved_deps.append(dep)

    def keep_readable(self, repo: Path, exclude: list[str]) -> None:
        """Keep the source files that exist and do not match `exclude`. A declared source
        that does not exist (generated, or missing) is recorded as a gap."""
        present = [f for f in self.files if f.exists()]
        if len(present) < len(self.files):
            missing = [str(f.relative_to(repo)) for f in self.files if f not in present]
            self.unit.not_measured.append(f"generated or missing sources {missing}")
        self.files = [f for f in present if not excluded(repo, f, exclude)]

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
    """Binaries in different packages may share a crate name; their keys must not."""
    count: dict[str, int] = defaultdict(int)
    for crate in crates:
        count[crate.name] += 1
    for crate in crates:
        if count[crate.name] > 1:
            crate.key = f"{crate.name}__{crate.unit.declared_at.rsplit('/', 1)[0].replace('/', '_')}"
            crate.unit.id = crate.key


def resolve_deps(crates: list[Crate]) -> None:
    by_id = {label: c.key for c in crates for label in c.unit.labels} | {c.name: c.key for c in crates}
    for crate in crates:
        for dep in crate.dep_ids:
            crate.add_dependency(dep, by_id.get(dep))
        crate.deps = sorted(set(crate.deps) - {crate.key})
