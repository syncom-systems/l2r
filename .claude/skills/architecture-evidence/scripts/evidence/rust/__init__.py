"""Rust: crates and their declared dependencies from the build (`bazel_rules`,
`cargo`, sharing `crates`), modules and their references from the source
(`source`).
"""

from __future__ import annotations

from pathlib import Path

from .. import bazel
from ..model import Dependency, Measurement, NoMeasurement, Unit
from . import bazel_rules, cargo
from . import crates as build
from .source import Observation

NOT_MEASURED = ["test targets, and test modules from the first #[cfg(test)] to the end of a file",
                "macro bodies and macro expansions",
                "a path resolves to the longest known module, so an item re-exported under its "
                "module's name lands on that module"]


class Workspace:
    """The Rust crates of a repository and their declared dependencies; on
    request, the source of some of them."""

    def __init__(self, repo: Path, crates: list[build.Crate], how: str, limits: list[str] | None = None):
        build.unique_keys(crates)
        build.resolve_deps(crates)
        self.repo = repo
        self.how = how
        self.not_measured = NOT_MEASURED + (limits or [])
        self.crates = {c.key: c for c in crates}

    @classmethod
    def load(cls, repo: Path) -> Workspace:
        if bazel.is_workspace(repo):
            crates = bazel_rules.from_bazel(repo, bazel.query(repo, bazel_rules.KINDS))
            how, limits = "bazel query (Bazel workspace; Cargo not consulted)", []
        else:
            crates, how, limits = cargo.from_cargo(repo), "Cargo.toml (no Bazel workspace)", cargo.NOT_MEASURED
        if not crates:
            raise NoMeasurement(f"no Rust crates found in {repo} by {how}")
        return cls(repo, crates, how, limits)

    def units(self) -> list[Unit]:
        return [c.unit for c in self.crates.values()]

    def declared(self) -> list[Dependency]:
        return [c.declared_dependency(d) for c in self.crates.values() for d in c.deps]

    def observe(self, keys: set[str], exclude: list[str]) -> Observation:
        """Modules and references of the crates `keys`, leaving out files matching `exclude`."""
        observation = Observation(self.repo, {k: self.crates[k] for k in sorted(keys)})
        observation.read(exclude)
        return observation

    def measure(self, keys: set[str], exclude: list[str]) -> Measurement:
        """Everything measured about the crates `keys`."""
        observation = self.observe(keys, exclude)
        return Measurement(measured_with=self.how,
                           units=[self.crates[k].unit for k in sorted(keys)] + observation.units(),
                           declared=self.declared(), observed=observation.dependencies,
                           undeclared=observation.undeclared, not_measured=self.not_measured)
