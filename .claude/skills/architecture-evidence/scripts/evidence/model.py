"""The evidence format: what was measured, with its provenance and its gaps,
and the few facts every analyzer shares (line numbers, left-out files).

This module owns the format. Language analyzers produce units and
dependencies; nothing else writes the JSON. The format records observations
only: a dependency is a fact about the build or the source, never a judgement
that one unit *uses* another in Parnas's sense.
"""

from __future__ import annotations

import fnmatch
from dataclasses import dataclass, field
from pathlib import Path

SCHEMA = "architecture-evidence/1"

# Dependency kinds.
DECLARED = "declared"   # the build declares it (Bazel `deps`, a Cargo dependency)
OBSERVED = "observed"   # the source refers to it (an import, a `use`, a qualified path)


class NoMeasurement(Exception):
    """The evidence is not available; nothing is written."""


def line_at(text: str, offset: int) -> int:
    """The 1-based line of `text` that `offset` falls on, for `file:line` evidence."""
    return text.count("\n", 0, offset) + 1


def readable(repo: Path, files: list[Path], exclude: list[str]) -> tuple[list[Path], list[str]]:
    """The declared source files to read, and those declared but missing (generated, or
    absent), relative to `repo`. A file matching an `exclude` glob is neither."""
    present = [f for f in files if f.exists()]
    missing = [str(f.relative_to(repo)) for f in files if f not in present]
    return [f for f in present if not excluded(repo, f, exclude)], missing


def excluded(repo: Path, file: Path, patterns: list[str]) -> bool:
    """True when `file` matches one of the globs the caller left out (e.g. generated code)."""
    return any(fnmatch.fnmatch(str(file.relative_to(repo)), p) for p in patterns)


@dataclass
class Unit:
    """A unit of code as the build or the language defines it: a build unit
    (a crate, a Bazel target) or a source unit inside one (a Rust module).

    It is not an architectural module. Which units make up a module of the
    design is decided by the design record, not here.
    """

    id: str
    kind: str
    build_unit: str | None = None     # for a source unit: the build unit it belongs to
    package: str = ""                 # the directory that declares a build unit
    declared_at: str = ""             # file:line of its declaration
    labels: list[str] = field(default_factory=list)
    testonly: bool = False
    visibility: list[str] = field(default_factory=list)
    roles: list[str] = field(default_factory=list)
    # Every declared dependency is in exactly one of these three.
    workspace_deps: list[str] = field(default_factory=list)   # build units of this workspace, by id
    external_deps: list[str] = field(default_factory=list)    # outside the workspace, as declared
    unresolved_deps: list[str] = field(default_factory=list)  # found nowhere, as declared
    not_measured: list[str] = field(default_factory=list)

    @property
    def is_build_unit(self) -> bool:
        return self.build_unit is None

    @property
    def owner(self) -> str:
        """The build unit this unit is, or belongs to."""
        return self.build_unit or self.id

    def to_json(self) -> dict:
        if not self.is_build_unit:
            return {"kind": self.kind, "build_unit": self.build_unit}
        return {"kind": self.kind, "roles": self.roles, "labels": self.labels,
                "declared_at": self.declared_at, "testonly": self.testonly,
                "visibility": self.visibility, "workspace_deps": sorted(self.workspace_deps),
                "external_deps": sorted(self.external_deps), "unresolved_deps": sorted(self.unresolved_deps),
                "not_measured": self.not_measured}


@dataclass(frozen=True)
class Dependency:
    kind: str       # DECLARED or OBSERVED
    source: str     # unit id
    target: str     # unit id
    evidence: str   # file:line and what is there

    @property
    def pair(self) -> tuple[str, str]:
        return self.source, self.target

    def to_json(self) -> dict:
        return {"kind": self.kind, "source": self.source, "target": self.target,
                "evidence": self.evidence}


@dataclass
class Measurement:
    """What one language analyzer reports for the measured build units: the units,
    both kinds of dependency, the references to undeclared build units and its gaps."""

    measured_with: str
    units: list[Unit]
    declared: list[Dependency]
    observed: list[Dependency]
    undeclared: list[str]
    not_measured: list[str]


@dataclass
class Evidence:
    """One measurement: the scope, its context and what was found."""

    measured_with: str
    scope: list[str]
    units: dict[str, Unit]
    dependencies: list[Dependency]
    loops: dict[str, list[list[str]]]
    checks: dict[str, list[str]]
    not_measured: list[str]

    def of_kind(self, kind: str) -> list[Dependency]:
        return [d for d in self.dependencies if d.kind == kind]

    def owner(self, unit_id: str) -> str:
        return self.units[unit_id].owner

    def to_json(self) -> dict:
        return {
            "schema": SCHEMA,
            "measured_with": self.measured_with,
            "scope": self.scope,
            "units": {key: self.units[key].to_json() for key in sorted(self.units)},
            "dependencies": [d.to_json() for d in self.dependencies],
            "loops": self.loops,
            "checks": self.checks,
            "not_measured": self.not_measured,
        }
