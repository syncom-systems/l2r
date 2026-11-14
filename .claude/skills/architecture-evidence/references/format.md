# evidence.json, `architecture-evidence/1`

One measurement of one scope. Every fact is an observation of the build or the source, with its provenance. Nothing in it is an architectural judgement.

Real output for `--scope crates` on this repository, abridged: two units and two dependencies of many are shown; `loops` and `not_measured` are complete.

```json
{
  "schema": "architecture-evidence/1",
  "measured_with": "bazel query (Bazel workspace; Cargo not consulted)",
  "scope": ["crates"],
  "units": {
    "l2r_validate": {
      "kind": "rust_crate",
      "roles": ["scope"],
      "labels": ["//crates/l2r_validate:l2r_validate"],
      "declared_at": "crates/l2r_validate/BUILD.bazel:3",
      "testonly": false,
      "visibility": ["//crates/l2r_apply:__pkg__", "//crates/l2r_cli:__pkg__", "//crates/l2r_plan:__pkg__"],
      "workspace_deps": ["l2r_model"],
      "external_deps": [],
      "unresolved_deps": [],
      "not_measured": []
    },
    "l2r_model::rust_names": {"kind": "rust_module", "build_unit": "l2r_model"}
  },
  "dependencies": [
    {"kind": "declared", "source": "l2r_validate", "target": "l2r_model",
     "evidence": "crates/l2r_validate/BUILD.bazel:3: //crates/l2r_validate:l2r_validate deps //crates/l2r_model:l2r_model"},
    {"kind": "observed", "source": "l2r_validate::flat", "target": "l2r_model::rust_names::composite_v2",
     "evidence": "crates/l2r_validate/src/flat.rs:13: use l2r_model::rust_names::composite_v2"}
  ],
  "loops": {
    "declared": [],
    "observed": [["l2r_apply::approval", "l2r_apply::assess", "l2r_apply::journal"],
                 ["l2r_apply::fault", "l2r_apply::tree"]]
  },
  "checks": {"observed_without_declared": [], "declared_without_observed": []},
  "not_measured": [
    "test targets, and test modules from the first #[cfg(test)] to the end of a file",
    "macro bodies and macro expansions",
    "a path resolves to the longest known module, so an item re-exported under its module's name lands on that module"
  ]
}
```

## Units

A **unit** is code as the build or the language delimits it. It is not an architectural module. Which units realise a module of a design is stated by the design record, not by the evidence.

- A **build unit** (no `build_unit` field) is what the build declares: a crate, a Bazel target. Only measured build units are listed: the scope and its context. Each carries:
  - `roles`: `scope`, `used_by_scope`, `uses_scope`;
  - its declaring `labels` and `declared_at`, and `testonly` (true only when every target merged into it is);
  - `visibility` as `bazel query` reports it, the union over merged targets. Empty means the target sets none and the package's default applies; the evidence does not resolve that default.
  - every declared dependency, in exactly one of `workspace_deps` (build units of this workspace, by id, whether measured or not), `external_deps` (another repository or registry) and `unresolved_deps` (found nowhere);
  - its own gaps, `not_measured`.
- A **source unit** names the build unit it belongs to in `build_unit`. A Rust crate's root module has the crate's id and is the build unit itself; its other modules are `crate::path`. Modules come from files: an inline module (`mod a { … }`) is not a unit of its own, a file module below it is (`k::a::b`), and a reference into an inline module lands on the nearest enclosing file module.

Kinds: `rust_crate`, `rust_module`.

## Dependencies

- **declared**: the build says so (a `deps` entry, a Cargo dependency). Between build units. Evidence cites the declaring line.
- **observed**: the source refers to it (a `use`, a qualified path). Between any units. Evidence cites the referring line. A unit's reference to its own ancestor is containment, not a dependency.

Only dependencies with an end in a scope unit are listed; a context unit's other declared dependencies are in its `workspace_deps`. Either kind is a compile-time fact. Whether it is a *use* (the source is correct only if the target is) is a judgement for the reader.

## Loops and checks

- `loops.declared`: groups of build units that reach one another through the listed declared dependencies. `loops.observed`: the same for the listed observed dependencies among all units.
- `checks.observed_without_declared`: a reference to a build unit the referring unit's build does not declare. The measurement is inconsistent (exit 1).
- `checks.declared_without_observed`: a listed declared dependency that no observed dependency matches, in what was read.

## Gaps

`not_measured` (global, and per build unit) states what was not read. A dependency missing from the evidence is absent only from what was read.

## Versions

`schema` changes when the meaning of a field changes or a field is removed. A consumer reads only the version it knows.
