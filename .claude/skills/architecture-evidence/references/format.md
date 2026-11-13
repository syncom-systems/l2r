# evidence.json, `architecture-evidence/1`

One measurement of one scope. Every fact is an observation of the build or the source, with its provenance. Nothing in it is an architectural judgement.

```json
{
  "schema": "architecture-evidence/1",
  "measured_with": "bazel query (Bazel workspace; Cargo not consulted)",
  "scope": ["crates"],
  "units": {
    "l2r_model": {
      "kind": "rust_crate",
      "roles": ["scope"],
      "labels": ["//crates/l2r_model:l2r_model"],
      "declared_at": "crates/l2r_model/BUILD.bazel:3",
      "testonly": false,
      "visibility": ["//crates:__subpackages__"],
      "external_deps": ["@crates//:serde", "@crates//:serde_json", "@crates//:sha2", "@crates//:toml"],
      "unresolved_deps": [],
      "not_measured": []
    },
    "l2r_model::rust_names": {"kind": "rust_module", "parent": "l2r_model"}
  },
  "dependencies": [
    {"kind": "declared", "source": "l2r_validate", "target": "l2r_model",
     "evidence": "crates/l2r_validate/BUILD.bazel:3: //crates/l2r_validate:l2r_validate deps //crates/l2r_model:l2r_model"},
    {"kind": "observed", "source": "l2r_validate::flat", "target": "l2r_model::rust_names::composite_v2",
     "evidence": "crates/l2r_validate/src/flat.rs:13: use l2r_model::rust_names::composite_v2"}
  ],
  "loops": {"declared": [], "observed": [["l2r_apply::fault", "l2r_apply::tree"]]},
  "checks": {"observed_without_declared": [], "declared_without_observed": []},
  "not_measured": ["macro bodies and macro expansions"]
}
```

## Units

A **unit** is code as the build or the language delimits it. It is not an architectural module. Which units realise a module of a design is stated by the design record, not by the evidence.

- A **build unit** (`parent` absent) is what the build declares: a crate, a Bazel target. It carries `roles` (`scope`, `used_by_scope`, `uses_scope`), its declaring `labels` and `declared_at`, `testonly`, `visibility` as the build reports it, the declared dependencies outside the measurement (`external_deps`) or not found anywhere (`unresolved_deps`), and its own gaps (`not_measured`).
- A **source unit** (`parent` set) lives inside a build unit. A Rust crate's root module has the crate's id; its other modules are `crate::path`.

Kinds: `rust_crate`, `rust_module`.

## Dependencies

- **declared**: the build says so (a `deps` entry, a Cargo dependency). Between build units. Evidence cites the declaring line.
- **observed**: the source refers to it (a `use`, a qualified path). Between any units. Evidence cites the referring line. A unit's reference to its own ancestor is containment, not a dependency.

Only dependencies with an end in the scope are listed. Either kind is a compile-time fact. Whether it is a *use* (the source is correct only if the target is) is a judgement for the reader.

## Loops and checks

- `loops.declared`: groups of build units that reach one another through declared dependencies. `loops.observed`: the same for observed dependencies among all units.
- `checks.observed_without_declared`: a reference to a build unit the referring unit's build does not declare. The measurement is inconsistent (exit 1).
- `checks.declared_without_observed`: a declared dependency between build units that no observed dependency matches, in what was read.

## Gaps

`not_measured` (global, and per build unit) states what was not read. A dependency missing from the evidence is absent only from what was read.

## Versions

`schema` changes when the meaning of a field changes or a field is removed. A consumer reads only the version it knows.
