"""Rust crates from Cargo manifests, read only when there is no Bazel
workspace: the package name and the `[dependencies]` table.
"""

from __future__ import annotations

import re
from pathlib import Path

from ..model import Unit, line_at
from .crates import Crate, crate_root

NOT_MEASURED = ["Cargo: only the [dependencies] table (no dev-, build- or target-specific "
                "dependencies, no workspace inheritance); one library or binary per manifest"]


def from_manifest(repo: Path, manifest: Path) -> Crate | None:
    text = manifest.read_text()
    name = re.search(r'^\[package\][^\[]*?^name\s*=\s*"([^"]+)"', text, re.M | re.S)
    files = sorted((manifest.parent / "src").rglob("*.rs"))
    if not name or not files:
        return None
    block = re.search(r"^\[dependencies\](.*?)(?=^\[|\Z)", text, re.M | re.S)
    deps = list(re.finditer(r"^([A-Za-z0-9_-]+)\s*=", block.group(1), re.M)) if block else []
    path, crate_name = manifest.relative_to(repo), name.group(1).replace("-", "_")
    unit = Unit(id=crate_name, kind="rust_crate", package=path.parent.as_posix(),
                declared_at=f"{path}:{line_at(text, name.start())}", labels=[str(path)])
    return Crate(name=crate_name, root=crate_root(files, None), files=files, unit=unit,
                 dep_ids=[d.group(1).replace("-", "_") for d in deps],
                 dep_at={d.group(1).replace("-", "_"): f"{path}:{line_at(text, block.start(1) + d.start())}"
                         for d in deps})


def build_output(repo: Path, path: Path) -> bool:
    """Inside `target/`, a Bazel output link, or a hidden directory."""
    return any(part == "target" or part.startswith((".", "bazel-")) for part in path.relative_to(repo).parts)


def from_cargo(repo: Path) -> list[Crate]:
    manifests = sorted(m for m in repo.rglob("Cargo.toml") if not build_output(repo, m))
    return [c for c in (from_manifest(repo, m) for m in manifests) if c]
