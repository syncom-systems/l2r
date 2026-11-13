"""Rust source: modules by file layout from each crate root, and the modules
that `use` statements and qualified paths refer to. A reference resolves to
the longest known module that prefixes its path.
"""

from __future__ import annotations

import re
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

from ..model import OBSERVED, Dependency, Unit, line_at, readable
from .crates import Crate

USE = re.compile(r"\buse\s+([^;]+);", re.S)
QUALIFIED = re.compile(r"\b(crate|super|self)((?:::[A-Za-z_][A-Za-z0-9_]*)+)")


def keep_readable(repo: Path, crate: Crate, exclude: list[str]) -> None:
    """Keep the crate's source files to read; record those declared but missing as a gap."""
    crate.files, missing = readable(repo, crate.files, exclude)
    if missing:
        crate.unit.not_measured.append(f"generated or missing sources {missing}")


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
    found = [(line_at(text, m.start()), path, f"use {m.group(1).strip()}")
             for m in USE.finditer(text) for path in flatten(m.group(1))]
    found += [(line_at(text, m.start()), [m.group(1), *m.group(2).split("::")[1:]], m.group(0))
              for m in QUALIFIED.finditer(text)]
    for name in sorted(crate_names):
        found += [(line_at(text, m.start()), m.group(0).split("::"), m.group(0))
                  for m in re.finditer(rf"(?<![\w:]){name}(?:::[A-Za-z_][A-Za-z0-9_]*)+", text)]
    return found


def absolute(path: list[str], here: list[str], names: dict[str, str]) -> list[str] | None:
    """The full module path a reference in module `here` means, or None for external
    and local names. `super` names the parent module whatever file `here` lives in."""
    head, rest = path[0], path[1:]
    if head == "crate":
        return [here[0], *rest]
    if head == "self":
        return [*here, *rest]
    if head == "super":
        return climb(here[:-1], rest)
    return [names[head], *rest] if head in names else None


def climb(parent: list[str], rest: list[str]) -> list[str]:
    """The module a `super::` path names, after any further `super` segments."""
    while rest and rest[0] == "super":
        parent, rest = parent[:-1], rest[1:]
    return [*parent, *rest]


def longest_module(path: list[str], known: dict[str, str]) -> str | None:
    """The longest known module that prefixes `path`."""
    return next(("::".join(path[:end]) for end in range(len(path), 0, -1) if "::".join(path[:end]) in known), None)


def is_dependency(source: str, target: str | None) -> bool:
    """A reference to another module that is not the referring module's own ancestor."""
    return bool(target) and not (source + "::").startswith(target + "::")


@dataclass
class Observation:
    """The modules of the measured crates and the references between them."""

    repo: Path
    crates: dict[str, Crate]
    modules: dict[str, str] = field(default_factory=dict)   # module id -> the id of its crate
    dependencies: list[Dependency] = field(default_factory=list)
    undeclared: list[str] = field(default_factory=list)

    def units(self) -> list[Unit]:
        """The modules other than crate roots; a crate's root module is the crate's unit."""
        return [Unit(id=m, kind="rust_module", build_unit=crate)
                for m, crate in sorted(self.modules.items()) if m != crate]

    def read(self, exclude: list[str]) -> None:
        for crate in self.crates.values():
            keep_readable(self.repo, crate, exclude)
        self.modules = {"::".join(module_of(c, f)): c.key for c in self.crates.values() for f in c.files}
        names: dict[str, list[str]] = defaultdict(list)
        for crate in self.crates.values():
            names[crate.name].append(crate.key)
        for crate in self.crates.values():
            visible = self.visible_names(crate, names)
            for file in crate.files:
                self.dependencies += self.file_dependencies(crate, file, visible)

    def visible_names(self, crate: Crate, all_names: dict[str, list[str]]) -> dict[str, str]:
        """Crate names this crate may refer to: its declared deps. Others are recorded if used."""
        names = {self.crates[d].name: d for d in crate.deps if d in self.crates}
        return names | {n: keys[0] for n, keys in all_names.items() if n not in names and n != crate.name}

    def file_dependencies(self, crate: Crate, file: Path, names: dict[str, str]) -> list[Dependency]:
        here = module_of(crate, file)
        source = "::".join(here)
        found: dict[str, Dependency] = {}
        for line, path, text in references(production_text(file), set(names)):
            target = longest_module(absolute(path, here, names) or [], self.modules)
            if not is_dependency(source, target):
                continue
            snippet = re.sub(r"\s+", " ", text)[:100]
            evidence = f"{file.relative_to(self.repo)}:{line}: {snippet}"
            if self.modules[target] not in [crate.key, *crate.deps]:
                self.undeclared.append(f"{source} -> {target}: {evidence}")
            found.setdefault(target, Dependency(OBSERVED, source, target, evidence))
        return list(found.values())
