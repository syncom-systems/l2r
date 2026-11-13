#!/usr/bin/env python3
"""Measure the dependency structure of Rust code, with evidence for every edge.

Read-only, standard library. What is measured is the scope plus its context:
the crates the scope depends on directly and the crates that depend on it
directly. No dependency is dropped without a record: every declared dependency
is a measured crate, an external crate, or listed as unresolved.

- Crates come from the build. In a Bazel workspace (MODULE.bazel or WORKSPACE)
  only from `bazel query` over the whole workspace; a failed or empty query is an
  error, never a fallback. Cargo manifests are read only when there is no Bazel
  workspace.
- Crate edges cite the declaring target (`BUILD.bazel:line`, the line where the
  target is declared; its `deps` list holds the label) or the manifest line.
- Module edges come from `use` statements and qualified paths (`crate::`,
  `super::`, `self::`, and the crates a crate declares), each with `file:line`.
  A module's reference to its own ancestor is containment, not an edge.
- Checks: a reference to a measured crate that the build does not declare is a
  measurement error (exit 1); a declared dependency with no reference is listed.

Limits, also written to edges.json as `not_measured`: test targets and test
modules (from the first `#[cfg(test)]` to the end of a file); macro bodies and
expansions; generated sources; `select()` branches; a path is resolved to the
longest known module, so a function re-exported under its module's name lands
on that module. Cargo: `[dependencies]` only, one library or binary per manifest.

    python3 deps.py --repo . --scope crates/foo [--exclude 'glob' ...] [--out architecture/audit]

Writes <out>/edges.json and <out>/measured.c4 (system `measured`), prints a summary.
Exit 0: measured. Exit 1: measured, but the checks found an inconsistency.
Exit 2: no measurement (missing tool, failed query, empty scope).
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import shutil
import subprocess
import xml.etree.ElementTree as ET
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

USE = re.compile(r"\buse\s+([^;]+);", re.S)
QUALIFIED = re.compile(r"\b(crate|super|self)((?:::[A-Za-z_][A-Za-z0-9_]*)+)")
ROOT_FILES = ("lib.rs", "main.rs")
RUST_KINDS = "rust_library|rust_binary|rust_proc_macro"
BAZEL_MARKERS = ("MODULE.bazel", "WORKSPACE", "WORKSPACE.bazel")


class NoMeasurement(Exception):
    """The evidence is not available; nothing is written."""


@dataclass
class Crate:
    key: str            # unique among measured crates; the module paths start with it
    name: str           # the Rust crate name, as code refers to it
    root: Path          # the crate root file (lib.rs, main.rs, crate_root)
    files: list[Path]
    declared_at: str    # file:line of the target or manifest
    labels: list[str] = field(default_factory=list)
    dep_ids: list[str] = field(default_factory=list)   # labels or names, as declared
    deps: list[str] = field(default_factory=list)      # keys of measured crates
    declared: dict[str, str] = field(default_factory=dict)  # dep key -> the label as declared
    dep_at: dict[str, str] = field(default_factory=dict)    # declared label -> file:line, when known
    external: list[str] = field(default_factory=list)
    unresolved: list[str] = field(default_factory=list)
    missing_srcs: list[str] = field(default_factory=list)
    testonly: bool = False
    configurable: bool = False
    roles: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class Edge:
    source: str
    target: str
    evidence: str

    def crates(self) -> tuple[str, str]:
        return self.source.split("::")[0], self.target.split("::")[0]


@dataclass
class Workspace:
    repo: Path
    crates: dict[str, Crate]
    how: str
    by_id: dict[str, str]   # declared label or name -> crate key


# ----------------------------------------------------------------- crates from the build


def run(cmd: list[str], cwd: Path) -> str:
    try:
        out = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600)
    except (OSError, subprocess.TimeoutExpired) as err:
        raise NoMeasurement(f"`{' '.join(cmd)}` could not run: {err}") from err
    if out.returncode != 0:
        raise NoMeasurement(f"`{' '.join(cmd)}` failed ({out.returncode}):\n{out.stderr.strip()[-2000:]}")
    return out.stdout


def crate_root(files: list[Path], declared: Path | None) -> Path:
    if declared:
        return declared
    roots = [f for f in files if f.name in ROOT_FILES]
    if roots:
        return roots[0]
    if len(files) == 1:
        return files[0]
    raise NoMeasurement(f"no crate root among {files[:3]}")


def label_path(repo: Path, label: str) -> Path:
    package, _, name = label.lstrip("/").partition(":")
    return repo / package / name


def xml_attr(rule: ET.Element, name: str) -> ET.Element | None:
    return next((a for a in rule if a.get("name") == name), None)


def xml_list(rule: ET.Element, name: str) -> list[str]:
    found = xml_attr(rule, name)
    return [item.get("value") for item in found] if found is not None else []


def xml_value(rule: ET.Element, name: str) -> str | None:
    found = xml_attr(rule, name)
    return found.get("value") if found is not None else None


def rust_sources(rule: ET.Element) -> list[str]:
    """Source labels in this workspace; a generated source is still listed, and reported later."""
    return [s for s in xml_list(rule, "srcs") if s.startswith("//") and s.endswith(".rs")]


def bazel_crate(repo: Path, rule: ET.Element) -> Crate | None:
    srcs = rust_sources(rule)
    if not srcs:
        return None
    files = [label_path(repo, s) for s in srcs]
    root = xml_value(rule, "crate_root")
    label = rule.get("name")
    name = (xml_value(rule, "crate_name") or label.rsplit(":", 1)[-1]).replace("-", "_")
    location = rule.get("location", "").removeprefix(f"{repo}/").rsplit(":", 1)[0]
    deps = xml_list(rule, "deps") + xml_list(rule, "proc_macro_deps")
    return Crate(key=name, name=name, root=crate_root(files, root and label_path(repo, root)),
                 files=files, declared_at=location, labels=[label], dep_ids=deps,
                 testonly=xml_value(rule, "testonly") in ("1", "true"),
                 configurable=any(a.get("configurable") == "yes" for a in rule))


def merge_variant(crates: dict[tuple[str, Path], Crate], crate: Crate) -> None:
    """Targets built from the same root under the same name are one crate (e.g. a feature variant)."""
    same = crates.setdefault((crate.name, crate.root), crate)
    if same is not crate:
        same.labels += crate.labels
        same.dep_ids = sorted(set(same.dep_ids) | set(crate.dep_ids))
        same.testonly = same.testonly and crate.testonly


def bazel_crates(repo: Path) -> list[Crate]:
    if not shutil.which("bazel"):
        raise NoMeasurement("this is a Bazel workspace, but `bazel` is not on PATH")
    text = run(["bazel", "query", f'kind("{RUST_KINDS}", //...)', "--output=xml"], repo)
    crates: dict[tuple[str, Path], Crate] = {}
    for rule in ET.fromstring(text).iter("rule"):
        crate = bazel_crate(repo, rule)
        if crate:
            merge_variant(crates, crate)
    return list(crates.values())


def cargo_crate(repo: Path, manifest: Path) -> Crate | None:
    text = manifest.read_text()
    name = re.search(r'^\[package\][^\[]*?^name\s*=\s*"([^"]+)"', text, re.M | re.S)
    files = sorted((manifest.parent / "src").rglob("*.rs"))
    if not name or not files:
        return None
    block = re.search(r"^\[dependencies\](.*?)(?=^\[|\Z)", text, re.M | re.S)
    deps = list(re.finditer(r"^([A-Za-z0-9_-]+)\s*=", block.group(1), re.M)) if block else []
    path, crate_name = manifest.relative_to(repo), name.group(1).replace("-", "_")
    line = lambda m: text.count("\n", 0, block.start(1) + m.start()) + 1
    return Crate(key=crate_name, name=crate_name, root=crate_root(files, None), files=files,
                 declared_at=f"{path}:{text.count(chr(10), 0, name.start()) + 1}", labels=[str(path)],
                 dep_ids=[d.group(1).replace("-", "_") for d in deps],
                 dep_at={d.group(1).replace("-", "_"): f"{path}:{line(d)}" for d in deps})


def build_output(repo: Path, path: Path) -> bool:
    """Inside `target/`, a Bazel output link, or a hidden directory."""
    return any(part == "target" or part.startswith((".", "bazel-")) for part in path.relative_to(repo).parts)


def cargo_crates(repo: Path) -> list[Crate]:
    manifests = sorted(m for m in repo.rglob("Cargo.toml") if not build_output(repo, m))
    return [c for c in (cargo_crate(repo, m) for m in manifests) if c]


def unique_keys(crates: list[Crate]) -> None:
    """Binaries in different packages may share a crate name; their keys must not."""
    count = defaultdict(int)
    for crate in crates:
        count[crate.name] += 1
    for crate in crates:
        if count[crate.name] > 1:
            crate.key = f"{crate.name}__{crate.declared_at.rsplit('/', 1)[0].replace('/', '_')}"


def is_external(dep: str) -> bool:
    """A Bazel label of another repository (`@crates//:serde`), or a Cargo name that is not a crate here."""
    return dep.startswith("@") or not any(ch in dep for ch in "/:")


def ids(crates: list[Crate]) -> dict[str, str]:
    """Every label and name a dependency may be declared by -> crate key."""
    by_label = {label: c.key for c in crates for label in c.labels}
    return by_label | {c.name: c.key for c in crates}


def resolve_deps(crates: list[Crate]) -> dict[str, str]:
    """Put every declared dependency in one bucket: a measured crate, external, or unresolved."""
    by_id = ids(crates)
    for crate in crates:
        for dep in crate.dep_ids:
            key = by_id.get(dep)
            if key:
                crate.declared.setdefault(key, dep)
            bucket = crate.deps if key else crate.external if is_external(dep) else crate.unresolved
            bucket.append(key or dep)
        crate.deps = sorted(set(crate.deps) - {crate.key})
    return by_id


def load_workspace(repo: Path) -> Workspace:
    if any((repo / m).exists() for m in BAZEL_MARKERS):
        crates, how = bazel_crates(repo), "bazel query (Bazel workspace; Cargo not consulted)"
    else:
        crates, how = cargo_crates(repo), "Cargo.toml (no Bazel workspace)"
    if not crates:
        raise NoMeasurement(f"no Rust crates found in {repo} by {how}")
    unique_keys(crates)
    by_id = resolve_deps(crates)
    return Workspace(repo, {c.key: c for c in crates}, how, by_id)


# ----------------------------------------------------------------- scope and context


def in_dirs(repo: Path, crate: Crate, dirs: list[str]) -> bool:
    rel = crate.root.relative_to(repo).as_posix()
    return any(rel.startswith(d.strip("/") + "/") for d in dirs)


def scope_keys(ws: Workspace, dirs: list[str]) -> set[str]:
    missing = [d for d in dirs if not (ws.repo / d).is_dir()]
    if missing:
        raise NoMeasurement(f"scope directory does not exist: {', '.join(missing)}")
    scope = {k for k, c in ws.crates.items() if in_dirs(ws.repo, c, dirs)}
    if not scope:
        raise NoMeasurement(f"no Rust crates under {dirs} ({ws.how})")
    return scope


def roles(ws: Workspace, key: str, scope: set[str]) -> list[str]:
    """scope, or the context: a crate the scope depends on directly, or one that depends on it directly."""
    if key in scope:
        return ["scope"]
    used = ["used_by_scope"] if any(key in ws.crates[s].deps for s in scope) else []
    return used + (["uses_scope"] if set(ws.crates[key].deps) & scope else [])


def keep_files(repo: Path, crate: Crate, exclude: list[str]) -> None:
    crate.missing_srcs = [str(f.relative_to(repo)) for f in crate.files if not f.exists()]
    crate.files = [f for f in crate.files if f.exists() and not excluded(repo, f, exclude)]


def select(ws: Workspace, dirs: list[str], exclude: list[str]) -> list[Crate]:
    scope = scope_keys(ws, dirs)
    for key, crate in ws.crates.items():
        crate.roles = roles(ws, key, scope)
    measured = [c for c in ws.crates.values() if c.roles]
    for crate in measured:
        keep_files(ws.repo, crate, exclude)
    return measured


def excluded(repo: Path, file: Path, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatch(str(file.relative_to(repo)), p) for p in patterns)


# ----------------------------------------------------------------- modules and their edges


def module_of(crate: Crate, file: Path) -> list[str]:
    if file == crate.root:
        return [crate.key]
    parts = list(file.relative_to(crate.root.parent).with_suffix("").parts)
    if parts[-1] == "mod":
        parts = parts[:-1]
    return [crate.key, *parts]


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
                  for m in re.finditer(rf"(?<![\w:]){name}(?:::[A-Za-z_][A-Za-z0-9_]*)+", text)]
    return found


def absolute(path: list[str], here: list[str], is_dir: bool, names: dict[str, str]) -> list[str] | None:
    """The full module path a reference means, or None for external and local names."""
    head, rest = path[0], path[1:]
    base = here if is_dir else here[:-1]
    if head == "crate":
        return [here[0], *rest]
    if head == "self":
        return [*here, *rest]
    if head == "super":
        return climb(base, rest, is_dir)
    return [names[head], *rest] if head in names else None


def climb(base: list[str], rest: list[str], is_dir: bool) -> list[str]:
    """The module a `super::` path names, after any further `super` segments."""
    while rest and rest[0] == "super":
        base, rest = base[:-1], rest[1:]
    return [*base[:-1], *rest] if is_dir else [*base, *rest]


def resolve(path: list[str], known: set[str]) -> str | None:
    """The longest known module that prefixes `path`."""
    return next(("::".join(path[:end]) for end in range(len(path), 0, -1) if "::".join(path[:end]) in known), None)


def is_dependency(source: str, target: str | None) -> bool:
    """A reference to another module that is not the referring module's own ancestor."""
    return bool(target) and not (source + "::").startswith(target + "::")


@dataclass
class Graph:
    """The measured crates (scope and context) and their modules."""
    repo: Path
    crates: dict[str, Crate]
    modules: set[str]
    undeclared: list[str] = field(default_factory=list)

    @property
    def scope(self) -> set[str]:
        return {k for k, c in self.crates.items() if "scope" in c.roles}

    def touches_scope(self, edge: Edge) -> bool:
        return bool(set(edge.crates()) & self.scope)

    def names(self) -> dict[str, list[str]]:
        """Crate name -> the keys of the measured crates with that name."""
        found: dict[str, list[str]] = defaultdict(list)
        for crate in self.crates.values():
            found[crate.name].append(crate.key)
        return found


def visible_names(graph: Graph, crate: Crate, all_names: dict[str, list[str]]) -> dict[str, str]:
    """Crate names this crate may refer to: its declared measured deps. Others are recorded if used."""
    names = {graph.crates[d].name: d for d in crate.deps if d in graph.crates}
    return names | {n: keys[0] for n, keys in all_names.items() if n not in names and n != crate.name}


def file_edges(graph: Graph, crate: Crate, file: Path, all_names: dict[str, list[str]]) -> list[Edge]:
    here = module_of(crate, file)
    source = "::".join(here)
    is_dir = file == crate.root or file.name == "mod.rs"
    names = visible_names(graph, crate, all_names)
    edges = {}
    for line, path, text in references(production_text(file), set(names)):
        target = resolve(absolute(path, here, is_dir, names) or [], graph.modules)
        if not is_dependency(source, target):
            continue
        snippet = re.sub(r"\s+", " ", text)[:100]
        evidence = f"{file.relative_to(graph.repo)}:{line}: {snippet}"
        if target.split("::")[0] not in [crate.key, *crate.deps]:
            graph.undeclared.append(f"{source} -> {target}: {evidence}")
        edges.setdefault(target, Edge(source, target, evidence))
    return list(edges.values())


def module_edges(graph: Graph) -> list[Edge]:
    names = graph.names()
    edges = [e for c in graph.crates.values() for f in c.files for e in file_edges(graph, c, f, names)]
    return [e for e in edges if graph.touches_scope(e)]


def dep_edge(crate: Crate, dep: str) -> Edge:
    label = crate.declared[dep]
    return Edge(crate.key, dep, f"{crate.dep_at.get(label, crate.declared_at)}: {crate.labels[0]} deps {label}")


def crate_edges(graph: Graph) -> list[Edge]:
    edges = [dep_edge(c, d) for c in graph.crates.values() for d in c.deps if d in graph.crates]
    return [e for e in edges if graph.touches_scope(e)]


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


def c4_path(name: str) -> str:
    crate = name.split("::")[0]
    return f"measured.{c4_id(crate)}" + (f".{c4_id(name)}" if "::" in name else "")


def c4_model(crates: list[Crate], modules: list[str], edges: list[Edge]) -> str:
    lines = ["// Generated by deps.py; rewritten on every run. Do not edit.", "model {",
             "  measured = system 'Measured' {"]
    for crate in crates:
        lines.append(f"    {c4_id(crate.key)} = crate '{crate.key}' {{")
        lines.append(f"      description '{', '.join(crate.roles)}'")
        lines += [f"      {c4_id(m)} = module '{m}'" for m in modules if m.startswith(crate.key + "::")]
        lines.append("    }")
    for edge in edges:
        lines.append(f"    {c4_path(edge.source)} -> {c4_path(edge.target)} {{")
        lines.append(f"      metadata {{\n        evidence '{edge.evidence.replace(chr(39), '`')}'\n      }}\n    }}")
    lines += ["  }", "}", "", "views {", "  view measured_crates {", "    title 'Crates (measured)'",
              "    include measured.*", "  }", "}", ""]
    return "\n".join(lines)


def not_measured(crates: list[Crate]) -> list[str]:
    notes = ["test targets, and test modules from the first #[cfg(test)] to the end of a file",
             "macro bodies and macro expansions"]
    notes += [f"{c.key}: generated or missing sources {c.missing_srcs}" for c in crates if c.missing_srcs]
    notes += [f"{c.key}: select() in its attributes; the union of the branches is measured"
              for c in crates if c.configurable]
    return notes


def crate_facts(crate: Crate) -> dict:
    return {"roles": crate.roles, "labels": crate.labels, "declared_at": crate.declared_at,
            "testonly": crate.testonly, "deps": crate.deps, "external_deps": sorted(crate.external),
            "unresolved_deps": sorted(crate.unresolved)}


def checks(graph: Graph, c_edges: list[Edge], m_edges: list[Edge]) -> dict:
    referenced = {e.crates() for e in m_edges}
    return {"references_to_undeclared_crates": sorted(set(graph.undeclared)),
            "declared_deps_without_reference": [f"{e.source} -> {e.target}" for e in c_edges
                                                if e.crates() not in referenced]}


def facts(how: str, graph: Graph, c_edges: list[Edge], m_edges: list[Edge]) -> dict:
    return {
        "crates_from": how,
        "crates": {key: crate_facts(crate) for key, crate in graph.crates.items()},
        "modules": sorted(graph.modules),
        "crate_edges": [e.__dict__ for e in c_edges],
        "module_edges": [e.__dict__ for e in m_edges],
        "crate_loops": loops([e.crates() for e in c_edges]),
        "module_loops": loops([(e.source, e.target) for e in m_edges]),
        "checks": checks(graph, c_edges, m_edges),
        "not_measured": not_measured(list(graph.crates.values())),
    }


def pictured_modules(graph: Graph, m_edges: list[Edge]) -> list[str]:
    """Every module of the scope, and the context modules that an edge touches."""
    ends = {m for e in m_edges for m in (e.source, e.target)}
    return sorted(ends | {m for m in graph.modules if m.split("::")[0] in graph.scope})


def measure(repo: Path, dirs: list[str], exclude: list[str]) -> tuple[dict, str]:
    ws = load_workspace(repo)
    crates = sorted(select(ws, dirs, exclude), key=lambda c: c.key)
    graph = Graph(repo, {c.key: c for c in crates}, {"::".join(module_of(c, f)) for c in crates for f in c.files})
    c_edges, m_edges = crate_edges(graph), module_edges(graph)
    found = facts(ws.how, graph, c_edges, m_edges)
    return found, c4_model(crates, pictured_modules(graph, m_edges), c_edges + m_edges)


def shown(value: object) -> object:
    return value or "none"


def summary(found: dict) -> list[str]:
    roles = defaultdict(list)
    for key, crate in found["crates"].items():
        for role in crate["roles"]:
            roles[role].append(key)
    unresolved = {k: c["unresolved_deps"] for k, c in found["crates"].items() if c["unresolved_deps"]}
    checks = found["checks"]
    return [f"crates from {found['crates_from']}",
            *(f"{role}: {', '.join(sorted(keys))}" for role, keys in sorted(roles.items())),
            f"{len(found['modules'])} modules, {len(found['crate_edges'])} crate edges, "
            f"{len(found['module_edges'])} module edges",
            f"crate loops: {shown(found['crate_loops'])}",
            f"module loops: {shown(found['module_loops'])}",
            f"unresolved declared deps: {shown(unresolved)}",
            f"references to undeclared crates: {shown(checks['references_to_undeclared_crates'])}",
            f"declared deps without a reference: {shown(checks['declared_deps_without_reference'])}",
            f"not measured: {'; '.join(found['not_measured'])}"]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", type=Path, default=Path("."))
    ap.add_argument("--scope", nargs="+", required=True, help="directories, relative to the repository")
    ap.add_argument("--exclude", nargs="*", default=[], help="globs of files to leave out, e.g. generated code")
    ap.add_argument("--out", type=Path, default=Path("architecture/audit"))
    args = ap.parse_args()
    repo = args.repo.resolve()
    try:
        found, model = measure(repo, args.scope, args.exclude)
    except NoMeasurement as err:
        print(f"deps.py: NO MEASUREMENT: {err}")
        return 2
    out = args.out if args.out.is_absolute() else repo / args.out
    out.mkdir(parents=True, exist_ok=True)
    (out / "edges.json").write_text(json.dumps(found, indent=2) + "\n")
    (out / "measured.c4").write_text(model)
    print("\n".join(summary(found)))
    print(f"wrote {out / 'edges.json'} and {out / 'measured.c4'}")
    if found["checks"]["references_to_undeclared_crates"]:
        print("deps.py: INCONSISTENT: code refers to crates its build does not declare; see edges.json checks")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
