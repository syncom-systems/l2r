"""Language-independent analysis of measured units: which build units are in
scope or its context, the loops among dependencies, and whether what the
source refers to agrees with what the build declares.
"""

from __future__ import annotations

from collections import defaultdict
from operator import attrgetter
from pathlib import Path

from .model import DECLARED, OBSERVED, Dependency, Evidence, Measurement, NoMeasurement, Unit


def in_dirs(package: str, dirs: list[str]) -> bool:
    """True when `package` is one of `dirs` or lies below one."""
    return any((package + "/").startswith(d.strip("/") + "/") for d in dirs)


def scope_of(repo: Path, units: list[Unit], dirs: list[str]) -> set[str]:
    """The build units declared in a package under `dirs`. None at all is no measurement."""
    missing = [d for d in dirs if not (repo / d).is_dir()]
    if missing:
        raise NoMeasurement(f"scope directory does not exist: {', '.join(missing)}")
    scope = {u.id for u in units if in_dirs(u.package, dirs)}
    if not scope:
        raise NoMeasurement(f"no build units under {dirs}")
    return scope


def assign_roles(units: list[Unit], declared: list[Dependency], scope: set[str]) -> set[str]:
    """Give each build unit its roles and return those measured: the scope, the units
    the scope declares directly (`used_by_scope`) and the units that declare the scope
    directly (`uses_scope`)."""
    targets: dict[str, set[str]] = defaultdict(set)
    for dep in declared:
        targets[dep.source].add(dep.target)
    for unit in units:
        unit.roles = roles(unit.id, scope, targets)
    return {u.id for u in units if u.roles}


def roles(key: str, scope: set[str], targets: dict[str, set[str]]) -> list[str]:
    if key in scope:
        return ["scope"]
    used = ["used_by_scope"] if any(key in targets[s] for s in scope) else []
    return used + (["uses_scope"] if targets[key] & scope else [])


def reachable(edges: dict[str, set[str]], start: str) -> set[str]:
    seen: set[str] = set()
    todo = list(edges.get(start, ()))
    while todo:
        node = todo.pop()
        if node not in seen:
            seen.add(node)
            todo.extend(edges.get(node, ()))
    return seen


def loops(deps: list[Dependency]) -> list[list[str]]:
    """Each group of units that reach one another."""
    edges: dict[str, set[str]] = defaultdict(set)
    for dep in deps:
        edges[dep.source].add(dep.target)
    reach = {v: reachable(edges, v) for v in list(edges)}
    groups = {frozenset(w for w in reach[v] if v in reach.get(w, ())) for v in reach if v in reach[v]}
    return sorted(sorted(g) for g in groups)


class Measured:
    """The measured units, and which build unit each belongs to."""

    def __init__(self, units: list[Unit]):
        self.units = {u.id: u for u in units}
        self.scope = {u.id for u in units if "scope" in u.roles}

    def build_pair(self, dep: Dependency) -> tuple[str, str]:
        return self.units[dep.source].owner, self.units[dep.target].owner

    def touches_scope(self, dep: Dependency) -> bool:
        return bool(set(self.build_pair(dep)) & self.scope)

    def declared(self, deps: list[Dependency]) -> list[Dependency]:
        """Declared dependencies between measured units that touch the scope."""
        kept = [d for d in deps if d.source in self.units and d.target in self.units]
        return sorted((d for d in kept if self.touches_scope(d)), key=attrgetter("pair"))


def assemble(measurement: Measurement, scope_dirs: list[str]) -> Evidence:
    """The evidence for the measured units: dependencies that touch the scope, their
    loops (declared among build units, observed among all units) and the checks."""
    measured = Measured(measurement.units)
    declared = measured.declared(measurement.declared)
    observed = [d for d in measurement.observed if measured.touches_scope(d)]
    return Evidence(
        measured_with=measurement.measured_with, scope=scope_dirs, units=measured.units,
        dependencies=declared + observed,
        loops={DECLARED: loops(declared), OBSERVED: loops(observed)},
        checks=checks(measured, declared, observed, measurement.undeclared),
        not_measured=measurement.not_measured)


def checks(measured: Measured, declared: list[Dependency], observed: list[Dependency],
           undeclared: list[str]) -> dict[str, list[str]]:
    """`observed_without_declared`: the source refers to a build unit its build does not
    declare, so the measurement is inconsistent. `declared_without_observed`: a declared
    dependency the measured source never refers to."""
    referenced = {measured.build_pair(d) for d in observed}
    return {"observed_without_declared": sorted(set(undeclared)),
            "declared_without_observed": [f"{d.source} -> {d.target}" for d in declared
                                          if d.pair not in referenced]}
