"""Known-answer tests of the evidence analyzer. Each test writes a small
repository to a temporary directory; nothing here is part of any build.

    python3 -m unittest discover -s .claude/skills/architecture-evidence/tests
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))

import measure as cli
from evidence import bazel, graph, rust
from evidence.model import DECLARED, OBSERVED, NoMeasurement


def write(root: Path, files: dict[str, str]) -> Path:
    for name, text in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(textwrap.dedent(text))
    return root


def manifest(name: str, *deps: str) -> str:
    lines = [f'[package]\nname = "{name}"\nversion = "0.1.0"\n\n[dependencies]']
    return "\n".join(lines + [f'{d} = {{ path = "../{d}" }}' if d != "serde" else 'serde = "1"' for d in deps]) + "\n"


# a declares b and serde; uses b, and c without declaring it.
# b declares c but never refers to it; b::x and b::y refer to each other.
CARGO = {
    "Cargo.toml": "[workspace]\nmembers = []\n",
    "crates/a/Cargo.toml": manifest("a", "b", "serde"),
    "crates/a/src/lib.rs": """\
        pub mod run;
        use b::x::{self, Thing as T};
        """,
    "crates/a/src/run.rs": """\
        use crate::helpers;
        pub fn go() { c::hidden(); super::run::go(); }
        #[cfg(test)]
        mod tests { use b::y; }
        """,
    "crates/a/src/helpers/mod.rs": "use super::model::Record;\npub fn h() {}\n",
    "crates/a/src/model.rs": "pub struct Record;\n",
    "crates/b/Cargo.toml": manifest("b", "c"),
    "crates/b/src/lib.rs": "pub mod x;\npub mod y;\n",
    "crates/b/src/x.rs": "use super::y::Other; // use c::ignored;\npub struct Thing;\n",
    "crates/b/src/y.rs": "pub struct Other(crate::x::Thing);\n",
    "crates/c/Cargo.toml": manifest("c"),
    "crates/c/src/lib.rs": "pub fn hidden() {}\n",
    "other/d/Cargo.toml": manifest("d", "a"),
    "other/d/src/main.rs": "fn main() { a::run::go(); }\n",
    "other/e/Cargo.toml": manifest("e"),
    "other/e/src/lib.rs": "\n",
}


def measure(repo: Path, scope: list[str]):
    return cli.measure(repo, scope, [])


class CargoWorkspace(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = write(Path(self.tmp.name), CARGO)

    def tearDown(self):
        self.tmp.cleanup()

    def pairs(self, evidence, kind):
        return {(d.source, d.target) for d in evidence.of_kind(kind)}

    def test_scope_and_context_roles(self):
        evidence = measure(self.repo, ["crates/a"])
        roles = {k: u.roles for k, u in evidence.units.items() if u.is_build_unit}
        self.assertEqual(roles, {"a": ["scope"], "b": ["used_by_scope"], "d": ["uses_scope"]})

    def test_declared_dependencies_cite_the_manifest_line(self):
        evidence = measure(self.repo, ["crates"])
        declared = {(d.source, d.target): d.evidence for d in evidence.of_kind(DECLARED)}
        self.assertEqual(set(declared), {("a", "b"), ("b", "c"), ("d", "a")})
        self.assertEqual(declared[("a", "b")], "crates/a/Cargo.toml:6: crates/a/Cargo.toml deps b")
        self.assertEqual(evidence.units["a"].external_deps, ["serde"])

    def test_observed_dependencies_resolve_use_trees_and_paths(self):
        evidence = measure(self.repo, ["crates"])
        observed = self.pairs(evidence, OBSERVED)
        self.assertLessEqual({("a", "b::x"), ("a::run", "a::helpers"), ("a::run", "c"),
                              ("b::x", "b::y"), ("b::y", "b::x")}, observed)
        # `super` names the parent module, from a `mod.rs` as from any other file.
        self.assertIn(("a::helpers", "a::model"), observed)
        # A module's reference to its own ancestor is containment, not a dependency.
        self.assertNotIn(("a::run", "a"), observed)

    def test_test_modules_and_comments_are_not_read(self):
        evidence = measure(self.repo, ["crates"])
        observed = self.pairs(evidence, OBSERVED)
        self.assertNotIn(("a::run", "b::y"), observed)
        self.assertNotIn(("b::x", "c"), observed)

    def test_loops_and_checks(self):
        evidence = measure(self.repo, ["crates"])
        self.assertEqual(evidence.loops[OBSERVED], [["b::x", "b::y"]])
        self.assertEqual(evidence.loops[DECLARED], [])
        self.assertEqual(len(evidence.checks["observed_without_declared"]), 1)
        self.assertIn("a::run -> c: crates/a/src/run.rs:2", evidence.checks["observed_without_declared"][0])
        self.assertEqual(evidence.checks["declared_without_observed"], ["b -> c"])

    def test_excluded_files_are_not_read(self):
        evidence = cli.measure(self.repo, ["crates"], ["crates/a/src/run.rs"])
        self.assertNotIn("a::run", evidence.units)
        self.assertEqual(evidence.checks["observed_without_declared"], [])

    def test_evidence_json_names_its_schema_and_keeps_kinds_apart(self):
        data = measure(self.repo, ["crates"]).to_json()
        self.assertEqual(data["schema"], "architecture-evidence/1")
        self.assertEqual({d["kind"] for d in data["dependencies"]}, {DECLARED, OBSERVED})
        self.assertEqual(data["units"]["b::x"], {"kind": "rust_module", "parent": "b"})

    def test_cli_exit_codes(self):
        out = Path(self.tmp.name) / "out"
        run = [sys.executable, str(SCRIPTS / "measure.py"), "--repo", str(self.repo), "--out", str(out)]
        inconsistent = subprocess.run(run + ["--scope", "crates"], capture_output=True, text=True)
        self.assertEqual(inconsistent.returncode, 1, inconsistent.stdout)
        self.assertEqual(json.loads((out / "evidence.json").read_text())["schema"], "architecture-evidence/1")
        self.assertIn("measured.b.b__x -> measured.b.b__y", (out / "measured.c4").read_text())
        consistent = subprocess.run(run + ["--scope", "other/e"], capture_output=True, text=True)
        self.assertEqual(consistent.returncode, 0, consistent.stdout)
        missing = subprocess.run(run + ["--scope", "nowhere"], capture_output=True, text=True)
        self.assertEqual(missing.returncode, 2)
        self.assertIn("NO MEASUREMENT", missing.stdout)


QUERY = """\
<?xml version="1.1" encoding="UTF-8" standalone="no"?>
<query version="2">
  <rule class="rust_library" location="{repo}/lib/BUILD.bazel:3:13" name="//lib:core">
    <list name="srcs"><label value="//lib:lib.rs"/><label value="//lib:gen.rs"/></list>
    <list name="deps"><label value="@crates//:serde"/><label value="//missing:thing"/></list>
    <list name="visibility"><label value="//app:__pkg__"/></list>
    <boolean name="testonly" value="false"/>
  </rule>
  <rule class="rust_library" location="{repo}/lib/BUILD.bazel:20:13" name="//lib:core_extra">
    <string name="crate_name" value="core"/>
    <label name="crate_root" value="//lib:lib.rs"/>
    <list name="srcs"><label value="//lib:lib.rs"/></list>
    <list name="visibility"><label value="//tools:__pkg__"/></list>
  </rule>
  <rule class="rust_binary" location="{repo}/app/BUILD.bazel:1:12" name="//app:app">
    <list name="srcs" configurable="yes"><label value="//app:main.rs"/></list>
    <list name="deps"><label value="//lib:core"/></list>
    <boolean name="testonly" value="true"/>
  </rule>
  <rule class="rust_library" location="{repo}/data/BUILD.bazel:1:12" name="//data:data">
    <list name="srcs"><label value="//data:schema.json"/></list>
  </rule>
</query>
"""


class BazelTargets(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = write(Path(self.tmp.name), {
            "MODULE.bazel": "",
            "lib/lib.rs": "pub fn f() {}\n",
            "app/main.rs": "fn main() { core::f(); }\n",
        })
        targets = bazel.parse(QUERY.format(repo=self.repo), self.repo)
        self.ws = rust.Workspace(self.repo, rust.bazel_rules.from_bazel(self.repo, targets), "test")

    def tearDown(self):
        self.tmp.cleanup()

    def test_targets_keep_location_visibility_and_testonly(self):
        targets = bazel.parse(QUERY.format(repo=self.repo), self.repo)
        core = targets[0]
        self.assertEqual((core.label, core.kind, core.declared_at, core.package),
                         ("//lib:core", "rust_library", "lib/BUILD.bazel:3", "lib"))
        self.assertFalse(core.testonly)
        self.assertTrue(targets[2].testonly and targets[2].configurable)

    def test_variants_of_one_crate_are_one_unit(self):
        self.assertEqual(sorted(self.ws.crates), ["app", "core"])
        core = self.ws.crates["core"].unit
        self.assertEqual(core.labels, ["//lib:core", "//lib:core_extra"])
        self.assertEqual(core.visibility, ["//app:__pkg__", "//tools:__pkg__"])

    def test_every_declared_dependency_lands_in_one_bucket(self):
        core = self.ws.crates["core"].unit
        self.assertEqual((core.external_deps, core.unresolved_deps), (["@crates//:serde"], ["//missing:thing"]))
        self.assertEqual([(d.source, d.target) for d in self.ws.declared()], [("app", "core")])

    def test_gaps_are_recorded_on_the_unit(self):
        scope = graph.scope_of(self.repo, self.ws.units(), ["app"])
        measured = graph.assign_roles(self.ws.units(), self.ws.declared(), scope)
        self.assertEqual(measured, {"app", "core"})
        observation = self.ws.observe(measured, [])
        self.assertEqual(self.ws.crates["app"].unit.not_measured,
                         ["select() in its attributes; the union of the branches is measured"])
        self.assertEqual(self.ws.crates["core"].unit.not_measured, ["generated or missing sources ['lib/gen.rs']"])
        self.assertEqual([(d.source, d.target) for d in observation.dependencies], [("app", "core")])

    def test_a_scope_without_build_units_is_no_measurement(self):
        (self.repo / "empty").mkdir()
        with self.assertRaises(NoMeasurement):
            graph.scope_of(self.repo, self.ws.units(), ["empty"])


if __name__ == "__main__":
    unittest.main()
