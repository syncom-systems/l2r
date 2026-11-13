#!/usr/bin/env python3
"""Measure the dependency structure of Rust code, with evidence for every edge.

Read-only. Standard library; `bazel` when the repository builds with Bazel,
otherwise the `Cargo.toml` files under the scope.

- Crate edges come from the build (Bazel `deps`, or Cargo path dependencies).
- Module edges come from `use` statements and qualified paths (`crate::`,
  `super::`, `self::`, other crates of the scope), each with `file:line`.
- Loops are reported for both. A module's reference to its own ancestor is
  containment, not an edge.

Limits: test modules (from the first `#[cfg(test)]` to the end of a file) and
macros are not read; a path is resolved to the longest known module, so an
edge to an item lands on the module that declares it, and a function
re-exported under its module's name lands on that module; testonly targets
count unless their files are excluded.

    python3 deps.py --repo . --scope crates [--exclude 'glob' ...] [--out architecture/audit]

Writes <out>/edges.json and <out>/measured.c4 (system `measured`), and prints a summary.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import shutil
import subprocess
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

USE = re.compile(r"\buse\s+([^;]+);", re.S)
QUALIFIED = re.compile(r"\b(crate|super|self)((?:::[A-Za-z_][A-Za-z0-9_]*)+)")
ROOT_FILES = ("lib.rs", "main.rs")


@dataclass
class Crate:
    name: str
    src: Path
    files: list[Path]
    deps: list[str] = field(default_factory=list)
    build_file: str = ""


@dataclass
class Scope:
    """What is measured: the crates, their modules, and how the crates were found."""
    repo: Path
    crates: list[Crate]
    how: str
    names: set[str] = field(default_factory=set)
    modules: set[str] = field(default_factory=set)


@dataclass(frozen=True)
class Edge:
    source: str
    target: str
    evidence: str


# ----------------------------------------------------------------- crates from the build


def run(cmd: list[str], cwd: Path) -> str:
    out = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600)
    return out.stdout if out.returncode == 0 else ""


def attr_list(block: str, attr: str) -> list[str]:
    found = re.search(rf"^\s*{attr} = \[([^\]]*)\]", block, re.M)
    return re.findall(r'"([^"]+)"', found.group(1)) if found else []


def attr_value(block: str, attr: str) -> str | None:
    found = re.search(rf'^\s*{attr} = "([^"]+)"', block, re.M)
    return found.group(1) if found else None


def src_root(files: list[Path]) -> Path:
    roots = [f.parent for f in files if f.name in ROOT_FILES]
    return roots[0] if roots else files[0].parent


def bazel_target(repo: Path, block: str) -> tuple[str, Crate] | None:
    name = attr_value(block, "name")
    srcs = [s for s in attr_list(block, "srcs") if s.startswith("//") and s.endswith(".rs")]
    if not name or not srcs:
        return None
    package = srcs[0][2:].split(":")[0]
    files = [repo / s[2:].replace(":", "/") for s in srcs]
    crate_name = (attr_value(block, "crate_name") or name).replace("-", "_")
    crate = Crate(crate_name, src_root(files), files, attr_list(block, "deps"), f"{package}/BUILD.bazel")
    return f"//{package}:{name}", crate


def label_key(label: str) -> str:
    return label if ":" in label else f"{label}:{label.rsplit('/', 1)[-1]}"


def bazel_crates(repo: Path, scope: list[str]) -> list[Crate]:
    pattern = " + ".join(f"//{s.strip('/')}/..." for s in scope)
    text = run(["bazel", "query", f'kind("rust_library|rust_binary", {pattern})', "--output=build"], repo)
    by_label: dict[str, str] = {}
    crates: dict[Path, Crate] = {}
    for block in re.split(r"\n(?=# )", text):
        target = bazel_target(repo, block)
        if target:
            by_label[target[0]] = target[1].name
            crates.setdefault(target[1].src, target[1])  # variants built from the same sources count once
    for crate in crates.values():
        crate.deps = sorted({by_label[label_key(d)] for d in crate.deps if label_key(d) in by_label} - {crate.name})
    return list(crates.values())


def cargo_crate(manifest: Path) -> Crate | None:
    text = manifest.read_text()
    name = re.search(r'^\[package\][^\[]*?^name\s*=\s*"([^"]+)"', text, re.M | re.S)
    files = sorted((manifest.parent / "src").rglob("*.rs"))
    if not name or not files:
        return None
    deps_block = re.search(r"^\[dependencies\](.*?)(?=^\[|\Z)", text, re.M | re.S)
    deps = re.findall(r"^([A-Za-z0-9_-]+)\s*=", deps_block.group(1), re.M) if deps_block else []
    return Crate(name.group(1).replace("-", "_"), src_root(files), files, [d.replace("-", "_") for d in deps],
                 str(manifest))


def cargo_crates(repo: Path, scope: list[str]) -> list[Crate]:
    manifests = [m for s in scope for m in sorted((repo / s).rglob("Cargo.toml"))]
    crates = [c for c in map(cargo_crate, manifests) if c]
    names = {c.name for c in crates}
    for crate in crates:
        crate.deps = sorted(set(crate.deps) & names - {crate.name})
    return crates


def find_crates(repo: Path, scope: list[str]) -> tuple[list[Crate], str]:
    if shutil.which("bazel") and (repo / "MODULE.bazel").exists():
        crates = bazel_crates(repo, scope)
        if crates:
            return crates, "bazel query"
    return cargo_crates(repo, scope), "Cargo.toml"


# ----------------------------------------------------------------- modules and their edges


def module_of(crate: Crate, file: Path) -> list[str]:
    parts = list(file.relative_to(crate.src).with_suffix("").parts)
    if parts[-1] in ("lib", "main", "mod"):
        parts = parts[:-1]
    return [crate.name, *parts]


def production_text(file: Path) -> str:
    text = file.read_text(errors="replace").split("#[cfg(test)]", 1)[0]
    return re.sub(r"//[^\n]*", "", text)


def flatten(tree: str) -> list[list[str]]:
    """Paths named by one `use` tree, e.g. `a::{b, c::d}` -> [a,b], [a,c,d]."""
    tree = re.sub(r"\s+", "", re.sub(r"\s+as\s+[A-Za-z_][A-Za-z0-9_]*", "", tree))
    if "{" not in tree:
        return [[p for p in tree.split("::") if p and p not in ("self", "*")]]
    head, _, rest = tree.partition("{")
    prefix = [p for p in head.split("::") if p]
    return [prefix + path for item in split_top(rest.rsplit("}", 1)[0]) for path in flatten(item)]


def split_top(inner: str) -> list[str]:
    """Split a brace group's contents at its top-level commas."""
    items, depth, current = [], 0, ""
    for ch in inner:
        depth += (ch == "{") - (ch == "}")
        if ch == "," and depth == 0:
            items.append(current)
            current = ""
        else:
            current += ch
    return [i for i in [*items, current] if i]


def references(text: str, crate_names: set[str]) -> list[tuple[int, list[str], str]]:
    """(line, path, source text) for every path that may name a module."""
    def line_of(pos: int) -> int:
        return text.count("\n", 0, pos) + 1

    found = [(line_of(m.start()), path, f"use {m.group(1).strip()}")
             for m in USE.finditer(text) for path in flatten(m.group(1))]
    found += [(line_of(m.start()), [m.group(1), *m.group(2).split("::")[1:]], m.group(0))
              for m in QUALIFIED.finditer(text)]
    for name in sorted(crate_names):
        found += [(line_of(m.start()), m.group(0).split("::"), m.group(0))
                  for m in re.finditer(rf"\b{name}(?:::[A-Za-z_][A-Za-z0-9_]*)+", text)]
    return found


def absolute(path: list[str], here: list[str], is_dir: bool, crate_names: set[str]) -> list[str] | None:
    """The full module path a reference means, or None for external and local names."""
    head, rest = path[0], path[1:]
    base = here if is_dir else here[:-1]
    if head == "crate":
        return [here[0], *rest]
    if head == "self":
        return [*base, *rest] if not is_dir else [*here, *rest]
    if head == "super":
        while rest and rest[0] == "super":
            base, rest = base[:-1], rest[1:]
        return [*base[:-1], *rest] if is_dir else [*base, *rest]
    return path if head in crate_names else None


def resolve(path: list[str], known: set[str]) -> str | None:
    """The longest known module that prefixes `path`."""
    return next(("::".join(path[:end]) for end in range(len(path), 0, -1) if "::".join(path[:end]) in known), None)


def is_dependency(source: str, target: str | None) -> bool:
    """A reference to another module that is not the referring module's own ancestor."""
    return bool(target) and not (source + "::").startswith(target + "::")


def file_edges(scope: Scope, crate: Crate, file: Path) -> list[Edge]:
    here = module_of(crate, file)
    source = "::".join(here)
    is_dir = file.name in ("lib.rs", "main.rs", "mod.rs")
    edges = {}
    for line, path, text in references(production_text(file), scope.names):
        target = resolve(absolute(path, here, is_dir, scope.names) or [], scope.modules)
        if is_dependency(source, target):
            evidence = f"{file.relative_to(scope.repo)}:{line}: {re.sub(r'\s+', ' ', text)[:100]}"
            edges.setdefault(target, Edge(source, target, evidence))
    return list(edges.values())


def excluded(repo: Path, file: Path, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatch(str(file.relative_to(repo)), p) for p in patterns)


# ----------------------------------------------------------------- graph


def reachable(edges: dict[str, set[str]], start: str) -> set[str]:
    seen: set[str] = set()
    todo = list(edges.get(start, ()))
    while todo:
        node = todo.pop()
        if node not in seen:
            seen.add(node)
            todo.extend(edges.get(node, ()))
    return seen


def loops(pairs: list[tuple[str, str]]) -> list[list[str]]:
    edges: dict[str, set[str]] = defaultdict(set)
    for source, target in pairs:
        edges[source].add(target)
    reach = {v: reachable(edges, v) for v in list(edges)}
    groups = {frozenset(w for w in reach[v] if v in reach.get(w, ())) for v in reach if v in reach[v]}
    return sorted(sorted(g) for g in groups)


# ----------------------------------------------------------------- output


def c4_id(name: str) -> str:
    return re.sub(r"[^A-Za-z0-9_]", "_", name.replace("::", "__"))


def c4_model(scope: Scope, crate_edges: list[Edge], module_edges: list[Edge]) -> str:
    lines = ["// Generated by deps.py; rewritten on every run. Do not edit.", "model {",
             "  measured = system 'Measured' {"]
    for crate in sorted(scope.crates, key=lambda c: c.name):
        lines.append(f"    {c4_id(crate.name)} = crate '{crate.name}' {{")
        lines += [f"      {c4_id(m)} = module '{m}'" for m in sorted(scope.modules) if m.startswith(crate.name + "::")]
        lines.append("    }")
    for edge in crate_edges + module_edges:
        lines.append(f"    {c4_path(edge.source)} -> {c4_path(edge.target)} {{")
        lines.append(f"      metadata {{\n        evidence '{edge.evidence.replace(chr(39), '`')}'\n      }}\n    }}")
    lines += ["  }", "}", "", "views {", "  view measured_crates {", "    title 'Crates (measured)'",
              "    include measured.*", "  }", "}", ""]
    return "\n".join(lines)


def c4_path(name: str) -> str:
    crate = name.split("::")[0]
    return f"measured.{c4_id(crate)}" + (f".{c4_id(name)}" if "::" in name else "")


def load_scope(repo: Path, dirs: list[str], exclude: list[str]) -> Scope:
    crates, how = find_crates(repo, dirs)
    for crate in crates:
        crate.files = [f for f in crate.files if f.exists() and not excluded(repo, f, exclude)]
    scope = Scope(repo, crates, how, names={c.name for c in crates})
    scope.modules = {"::".join(module_of(c, f)) for c in crates for f in c.files}
    return scope


def facts(scope: Scope, crate_edges: list[Edge], module_edges: list[Edge]) -> dict:
    return {
        "crates_from": scope.how,
        "crates": {c.name: c.deps for c in sorted(scope.crates, key=lambda c: c.name)},
        "modules": sorted(scope.modules),
        "crate_edges": [e.__dict__ for e in crate_edges],
        "module_edges": [e.__dict__ for e in module_edges],
        "crate_loops": loops([(e.source, e.target) for e in crate_edges]),
        "module_loops": loops([(e.source, e.target) for e in module_edges]),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", type=Path, default=Path("."))
    ap.add_argument("--scope", nargs="+", required=True, help="directories, relative to the repository")
    ap.add_argument("--exclude", nargs="*", default=[], help="globs of files to leave out, e.g. generated code")
    ap.add_argument("--out", type=Path, default=Path("architecture/audit"))
    args = ap.parse_args()
    scope = load_scope(args.repo.resolve(), args.scope, args.exclude)
    crate_edges = [Edge(c.name, d, c.build_file) for c in scope.crates for d in c.deps]
    module_edges = [e for c in scope.crates for f in c.files for e in file_edges(scope, c, f)]
    found = facts(scope, crate_edges, module_edges)
    out = args.out if args.out.is_absolute() else scope.repo / args.out
    out.mkdir(parents=True, exist_ok=True)
    (out / "edges.json").write_text(json.dumps(found, indent=2) + "\n")
    (out / "measured.c4").write_text(c4_model(scope, crate_edges, module_edges))
    print(f"{len(scope.crates)} crates ({scope.how}), {len(scope.modules)} modules, "
          f"{len(crate_edges)} crate edges, {len(module_edges)} module edges")
    print(f"crate loops: {found['crate_loops'] or 'none'}")
    print(f"module loops: {found['module_loops'] or 'none'}")
    print(f"wrote {out / 'edges.json'} and {out / 'measured.c4'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
