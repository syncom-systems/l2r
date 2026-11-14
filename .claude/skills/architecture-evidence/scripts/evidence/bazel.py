"""Build units from a Bazel workspace: one `bazel query` over the whole
workspace, parsed from its XML output. Language-independent; a language
analyzer names the rule kinds it measures and reads the attributes it needs.
"""

from __future__ import annotations

import shutil
import subprocess
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

from .model import NoMeasurement

MARKERS = ("MODULE.bazel", "WORKSPACE", "WORKSPACE.bazel")


def is_workspace(repo: Path) -> bool:
    return any((repo / marker).exists() for marker in MARKERS)


@dataclass
class Target:
    label: str
    kind: str
    declared_at: str            # BUILD file:line, relative to the repository
    lists: dict[str, list[str]]
    values: dict[str, str]
    configurable: bool          # some attribute uses select()

    @property
    def package(self) -> str:
        return self.label.lstrip("/").partition(":")[0]

    @property
    def testonly(self) -> bool:
        return self.values.get("testonly") in ("1", "true")

    def labels(self, name: str) -> list[str]:
        return self.lists.get(name, [])

    def sources(self, suffix: str) -> list[str]:
        """Source labels of this workspace ending in `suffix`; a generated one is still listed."""
        return [s for s in self.labels("srcs") if s.startswith("//") and s.endswith(suffix)]


def label_path(repo: Path, label: str) -> Path:
    package, _, name = label.lstrip("/").partition(":")
    return repo / package / name


def run(cmd: list[str], cwd: Path) -> str:
    try:
        out = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=600)
    except (OSError, subprocess.TimeoutExpired) as err:
        raise NoMeasurement(f"`{' '.join(cmd)}` could not run: {err}") from err
    if out.returncode != 0:
        raise NoMeasurement(f"`{' '.join(cmd)}` failed ({out.returncode}):\n{out.stderr.strip()[-2000:]}")
    return out.stdout


def query(repo: Path, kinds: list[str]) -> list[Target]:
    """Every target of the given rule kinds in the workspace. A failed query is an error."""
    if not shutil.which("bazel"):
        raise NoMeasurement("this is a Bazel workspace, but `bazel` is not on PATH")
    text = run(["bazel", "query", f'kind("{"|".join(kinds)}", //...)', "--output=xml"], repo)
    return parse(text, repo)


def parse(xml_text: str, repo: Path) -> list[Target]:
    return [target(rule, repo) for rule in ET.fromstring(xml_text).iter("rule")]


def target(rule: ET.Element, repo: Path) -> Target:
    lists = {a.get("name"): [item.get("value") for item in a] for a in rule if a.tag == "list"}
    values = {a.get("name"): a.get("value") for a in rule if a.tag != "list" and a.get("value") is not None}
    location = rule.get("location", "")
    location = location[len(f"{repo}/"):] if location.startswith(f"{repo}/") else location
    return Target(label=rule.get("name"), kind=rule.get("class", ""),
                  declared_at=location.rsplit(":", 1)[0], lists=lists, values=values,
                  configurable=any(a.get("configurable") == "yes" for a in rule))
