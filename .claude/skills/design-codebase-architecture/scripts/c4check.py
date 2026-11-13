#!/usr/bin/env python3
"""Check that the design picture (LikeC4) agrees with the design record.

Read-only. Standard library plus the `likec4` CLI (`brew install likec4`).

1. `likec4 validate <workspace>` must pass.
2. Over the model relations of the `designed` system (the "uses" relation):
   - loops (Parnas 1979: the uses relation must be loop-free);
   - the levels that follow from the relation (level 0 uses nothing);
   - relations whose `entry` metadata is missing;
   - with --record, entries the record (design.md) does not contain.

It does not judge whether the design is good; that is the architect's call.
Exit 0 when the model is valid and nothing above is found, 1 otherwise.

    python3 c4check.py architecture/ [--record architecture/design.md] [--system designed] [--json]
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path


def likec4(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(["likec4", *args], capture_output=True, text=True, timeout=600)


def validate(workspace: Path) -> list[str]:
    out = likec4(["validate", str(workspace)])
    text = re.sub(r"\x1b\[[0-9;]*m", "", out.stdout + out.stderr)
    if out.returncode == 0 and "Invalid" not in text:
        return []
    lines = [line.strip() for line in text.splitlines() if any(mark in line for mark in ("Line", "Invalid", "ERROR"))]
    return lines or [text.strip()[-2000:]]


def export(workspace: Path) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        target = Path(tmp) / "model.json"
        out = likec4(["export", "json", "-o", str(target), str(workspace)])
        if out.returncode != 0 or not target.exists():
            raise RuntimeError((out.stdout + out.stderr).strip()[-2000:])
        return json.loads(target.read_text())


def reachable(edges: dict[str, set[str]], start: str) -> set[str]:
    """Every node reachable from `start` by one or more uses edges."""
    seen: set[str] = set()
    todo = list(edges.get(start, ()))
    while todo:
        node = todo.pop()
        if node not in seen:
            seen.add(node)
            todo.extend(edges.get(node, ()))
    return seen


def loops(reach: dict[str, set[str]]) -> list[list[str]]:
    """Groups of modules that use each other, directly or through others."""
    groups = {frozenset(w for w in reach[v] if v in reach[w]) for v in reach if v in reach[v]}
    return sorted(sorted(group) for group in groups)


def levels(edges: dict[str, set[str]], reach: dict[str, set[str]], members: set[str]) -> dict[int, list[str]]:
    """Parnas's levels of `members`: 0 uses nothing; i uses something on i-1 and
    nothing higher. A module in a loop, or using one, gets level -1."""
    looped = {v for v in reach if v in reach[v]}
    level: dict[str, int] = {}

    def of(v: str) -> int:
        if v not in level:
            level[v] = 1 + max((of(w) for w in edges.get(v, ())), default=-1)
        return level[v]

    by_level: dict[int, list[str]] = defaultdict(list)
    for v in sorted(members):
        by_level[-1 if (reach[v] | {v}) & looped else of(v)].append(v)
    return dict(sorted(by_level.items()))


def record_ids(record: Path) -> set[str]:
    """Uses-edge ids that head a table row in the record (`| U3 | ...`)."""
    return set(re.findall(r"^\|\s*(U\d+)\s*\|", record.read_text(), re.M))


def uses_relations(model: dict, system: str) -> list[tuple[str, str, str, str | None]]:
    """(label, source, target, entry) for each relation touching `system`."""
    prefix = system + "."
    found = []
    for rel in model.get("relations", {}).values():
        src, dst = rel["source"]["model"], rel["target"]["model"]
        if src.startswith(prefix) or dst.startswith(prefix):
            label = f"{src} -> {dst}" + (f" '{rel['title']}'" if rel.get("title") else "")
            found.append((label, src, dst, (rel.get("metadata") or {}).get("entry")))
    return found


def entry_problem(entry: str | None, known: set[str] | None) -> tuple[str, str] | None:
    """Why a relation's `entry` does not point at a uses edge in the record, if it does not."""
    if not entry:
        return ("missing_entry", "")
    if not re.fullmatch(r"U\d+", entry):
        return ("unknown_entry", f" (entry {entry} is not a uses edge U<n>)")
    if known is not None and entry not in known:
        return ("unknown_entry", f" (entry {entry})")
    return None


def check(model: dict, system: str, record: Path | None) -> dict:
    known = record_ids(record) if record else None
    findings: dict = {"loops": [], "missing_entry": [], "unknown_entry": [], "levels": {}}
    edges: dict[str, set[str]] = defaultdict(set)
    members = {k for k in model.get("elements", {}) if k.startswith(system + ".")}
    nodes = set(members)
    for label, src, dst, entry in uses_relations(model, system):
        problem = entry_problem(entry, known)
        if problem:
            findings[problem[0]].append(label + problem[1])
        if not (dst.startswith(src + ".") or src.startswith(dst + ".")):  # containment is not use
            edges[src].add(dst)
            nodes.update((src, dst))  # external endpoints too, so loops through them are found
    reach = {v: reachable(edges, v) for v in nodes}
    findings["loops"] = loops(reach)
    findings["levels"] = levels(edges, reach, members)
    return findings


PROBLEMS = [
    ("loops", "loops in the uses relation"),
    ("missing_entry", "relations naming no record entry"),
    ("unknown_entry", "relations naming an entry the record lacks"),
]


def report(findings: dict, system: str) -> None:
    lines = ["valid LikeC4 model"]
    for key, label in PROBLEMS:
        lines.append(f"{label}: {len(findings[key])}")
        lines.extend(f"  - {item}" for item in findings[key][:40])
    lines.append("levels:")
    lines.extend(
        f"  {'in a loop' if level < 0 else level}: {', '.join(m.removeprefix(system + '.') for m in members)}"
        for level, members in findings["levels"].items()
    )
    print("\n".join(lines))


def arguments() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("workspace", type=Path, help="directory holding the .c4 files")
    ap.add_argument("--record", type=Path, help="the design record (design.md) whose entry ids relations must cite")
    ap.add_argument("--system", default="designed", help="top-level system to check (default: designed)")
    ap.add_argument("--json", action="store_true")
    return ap.parse_args()


def main() -> int:
    args = arguments()
    if not shutil.which("likec4"):
        print("likec4 is not installed: brew install likec4", file=sys.stderr)
        return 2
    errors = validate(args.workspace)
    if errors:
        print("INVALID LikeC4 model:\n  " + "\n  ".join(errors))
        return 1
    findings = check(export(args.workspace), args.system, args.record)
    if args.json:
        print(json.dumps({"valid": True, **findings}, indent=2))
    else:
        report(findings, args.system)
    return 1 if any(findings[key] for key, _ in PROBLEMS) else 0


if __name__ == "__main__":
    sys.exit(main())
