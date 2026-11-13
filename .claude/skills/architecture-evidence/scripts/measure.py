#!/usr/bin/env python3
"""Measure the dependency structure of code, with evidence for every dependency.

Read-only, standard library. What is measured is the scope plus its context:
the build units the scope declares directly and the build units that declare
the scope directly. The scope is the build units declared in a package at or
below one of the scope directories. Every declared dependency of a measured
build unit is recorded on it: a build unit of the workspace, external, or
unresolved.

Languages: Rust (crates from Bazel, or from Cargo when there is no Bazel
workspace; modules and references from the source).

    python3 measure.py --repo . --scope crates/foo [--exclude 'glob' ...] [--out architecture/audit]

Writes <out>/evidence.json (format: references/format.md) and <out>/measured.c4
(system `measured`), and prints a summary.
Exit 0: measured. Exit 1: measured, but the source refers to a build unit its
build does not declare. Exit 2: no measurement (missing tool, failed query,
empty scope, output not writable). Exit 3: a defect of this tool; nothing is
written.
"""

from __future__ import annotations

import argparse
import json
import traceback
from collections import defaultdict
from pathlib import Path

from evidence import c4, graph, rust
from evidence.model import DECLARED, OBSERVED, Evidence, NoMeasurement


def measure(repo: Path, dirs: list[str], exclude: list[str]) -> Evidence:
    ws = rust.Workspace.load(repo)
    scope = graph.scope_of(repo, ws.units(), dirs)
    measured = graph.assign_roles(ws.units(), ws.declared(), scope)
    return graph.assemble(ws.measure(measured, exclude), dirs)


def shown(value: object) -> object:
    return value or "none"


def summary(evidence: Evidence) -> list[str]:
    roles = defaultdict(list)
    for key, unit in evidence.units.items():
        for role in unit.roles:
            roles[role].append(key)
    build_units = [u for u in evidence.units.values() if u.is_build_unit]
    unresolved = {u.id: u.unresolved_deps for u in build_units if u.unresolved_deps}
    gaps = [f"{u.id}: {note}" for u in build_units for note in u.not_measured]
    return [f"build units from {evidence.measured_with}",
            *(f"{role}: {', '.join(sorted(keys))}" for role, keys in sorted(roles.items())),
            f"{len(evidence.units) - len(build_units)} source units, "
            f"{len(evidence.of_kind(DECLARED))} declared and {len(evidence.of_kind(OBSERVED))} observed dependencies",
            f"declared loops: {shown(evidence.loops[DECLARED])}",
            f"observed loops: {shown(evidence.loops[OBSERVED])}",
            f"unresolved declared deps: {shown(unresolved)}",
            f"observed without declared: {shown(evidence.checks['observed_without_declared'])}",
            f"declared without observed: {shown(evidence.checks['declared_without_observed'])}",
            f"not measured: {'; '.join(evidence.not_measured + gaps)}"]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", type=Path, default=Path("."))
    ap.add_argument("--scope", nargs="+", required=True, help="directories, relative to the repository")
    ap.add_argument("--exclude", nargs="*", default=[], help="globs of files to leave out, e.g. generated code")
    ap.add_argument("--out", type=Path, default=Path("architecture/audit"))
    args = ap.parse_args()
    repo = args.repo.resolve()
    try:
        evidence = measure(repo, args.scope, args.exclude)
        data, model = json.dumps(evidence.to_json(), indent=2) + "\n", c4.model(evidence)
    except NoMeasurement as err:
        print(f"measure.py: NO MEASUREMENT: {err}")
        return 2
    except Exception:  # a defect of this tool, not a fact about the code; never exit 1 for it
        traceback.print_exc()
        print("measure.py: INTERNAL ERROR: nothing written; this is a defect of the analyzer")
        return 3
    out = args.out if args.out.is_absolute() else repo / args.out
    try:
        out.mkdir(parents=True, exist_ok=True)
        (out / "evidence.json").write_text(data)
        (out / "measured.c4").write_text(model)
    except OSError as err:
        print(f"measure.py: NO MEASUREMENT: could not write {out}: {err}")
        return 2
    print("\n".join(summary(evidence)))
    print(f"wrote {out / 'evidence.json'} and {out / 'measured.c4'}")
    if evidence.checks["observed_without_declared"]:
        print("measure.py: INCONSISTENT: source refers to build units its build does not declare; "
              "see evidence.json checks")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
