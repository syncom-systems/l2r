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
    lines = [l.strip() for l in text.splitlines() if "Line" in l or "Invalid" in l or "ERROR" in l]
    return lines or [text.strip()[-2000:]]


def export(workspace: Path) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        target = Path(tmp) / "model.json"
        out = likec4(["export", "json", "-o", str(target), str(workspace)])
        if out.returncode != 0 or not target.exists():
            raise RuntimeError((out.stdout + out.stderr).strip()[-2000:])
        return json.loads(target.read_text())


def loops(edges: dict[str, set[str]]) -> list[list[str]]:
    """Strongly connected components with more than one node, plus self-loops."""
    index: dict[str, int] = {}
    low: dict[str, int] = {}
    stack: list[str] = []
    on: set[str] = set()
    found: list[list[str]] = []
    counter = [0]
    sys.setrecursionlimit(10000)

    def visit(v: str) -> None:
        index[v] = low[v] = counter[0]
        counter[0] += 1
        stack.append(v)
        on.add(v)
        for w in edges.get(v, ()):
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = []
            while True:
                w = stack.pop()
                on.discard(w)
                comp.append(w)
                if w == v:
                    break
            if len(comp) > 1 or v in edges.get(v, ()):
                found.append(sorted(comp))

    for node in list(edges):
        if node not in index:
            visit(node)
    return found


def levels(nodes: set[str], edges: dict[str, set[str]]) -> dict[int, list[str]]:
    """Parnas's levels: 0 uses nothing; i uses something on i-1 and nothing higher."""
    level: dict[str, int] = {}

    def of(v: str, seen: frozenset) -> int:
        if v in level:
            return level[v]
        if v in seen:
            return -1  # inside a loop; reported separately
        used = [of(w, seen | {v}) for w in edges.get(v, ())]
        value = -1 if any(u < 0 for u in used) else (max(used) + 1 if used else 0)
        level[v] = value
        return value

    by_level: dict[int, list[str]] = defaultdict(list)
    for v in sorted(nodes):
        by_level[of(v, frozenset())].append(v)
    return dict(sorted(by_level.items()))


def record_ids(record: Path) -> set[str]:
    """Uses-edge ids that head a table row in the record (`| U3 | ...`)."""
    return set(re.findall(r"^\|\s*(U\d+)\s*\|", record.read_text(), re.M))


def check(model: dict, system: str, record: Path | None) -> dict:
    prefix = system + "."
    nodes = {k for k in model.get("elements", {}) if k.startswith(prefix)}
    edges: dict[str, set[str]] = defaultdict(set)
    findings: dict = {"loops": [], "missing_entry": [], "unknown_entry": [], "levels": {}}
    known = record_ids(record) if record else None
    for rel in model.get("relations", {}).values():
        src, dst = rel["source"]["model"], rel["target"]["model"]
        if not (src.startswith(prefix) or dst.startswith(prefix)):
            continue
        name = f"{src} -> {dst}" + (f" '{rel['title']}'" if rel.get("title") else "")
        entry = (rel.get("metadata") or {}).get("entry")
        if not entry:
            findings["missing_entry"].append(name)
        elif not re.fullmatch(r"U\d+", entry):
            findings["unknown_entry"].append(f"{name} (entry {entry} is not a uses edge U<n>)")
        elif known is not None and entry not in known:
            findings["unknown_entry"].append(f"{name} (entry {entry})")
        if dst.startswith(src + ".") or src.startswith(dst + "."):
            continue  # containment, not use
        edges[src].add(dst)
        nodes.update(n for n in (src, dst) if n.startswith(prefix))
    findings["loops"] = loops(edges)
    findings["levels"] = {str(k) if k >= 0 else "in a loop": v for k, v in levels(nodes, edges).items()}
    return findings


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("workspace", help="directory holding the .c4 files")
    ap.add_argument("--record", type=Path, help="the design record (design.md) whose entry ids relations must cite")
    ap.add_argument("--system", default="designed", help="top-level system to check (default: designed)")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    if not shutil.which("likec4"):
        print("likec4 is not installed: brew install likec4", file=sys.stderr)
        return 2
    errors = validate(Path(args.workspace))
    if errors:
        print("INVALID LikeC4 model:\n  " + "\n  ".join(errors))
        return 1
    findings = check(export(Path(args.workspace)), args.system, args.record)
    failing = findings["loops"] or findings["missing_entry"] or findings["unknown_entry"]
    if args.json:
        print(json.dumps({"valid": True, **findings}, indent=2))
    else:
        print("valid LikeC4 model")
        for key, label in [("loops", "loops in the uses relation"),
                           ("missing_entry", "relations naming no record entry"),
                           ("unknown_entry", "relations naming an entry the record lacks")]:
            print(f"{label}: {len(findings[key])}")
            for item in findings[key][:40]:
                print(f"  - {item}")
        print("levels:")
        for level, members in findings["levels"].items():
            print(f"  {level}: {', '.join(m.removeprefix(args.system + '.') for m in members)}")
    return 1 if failing else 0


if __name__ == "__main__":
    sys.exit(main())
