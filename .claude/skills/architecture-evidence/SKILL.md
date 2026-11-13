---
name: architecture-evidence
description: Measure the dependency structure of existing code as evidence, without interpreting it. Reads build units (crates, Bazel targets) and their declared dependencies and visibility from the build, source units and their references from the code, each with file:line provenance, plus loops, declared-versus-observed checks and what was not measured. Writes evidence.json and a LikeC4 model. Use whenever an audit, a design review or a verification needs measured dependencies, cycles or layering facts about a codebase, or someone asks what depends on what. Interpreting the evidence is the job of audit-codebase-architecture and arch-verify.
---

# Architecture evidence

Collect facts about how code depends on code, with provenance and with the gaps stated. This skill observes. It does not judge. Whether a dependency is a *use* in Parnas's sense, or a breach of anything, is decided by the skill that reads the evidence, against the knowledge base.

## Run it

```bash
python3 <skill>/scripts/measure.py --repo . --scope <dirs> [--exclude '<generated glob>'] --out <dir>
```

Standard-library Python 3.9 or later; read-only except for `--out`. It measures the scope together with its context: the build units the scope declares directly (`used_by_scope`) and the build units that declare the scope directly (`uses_scope`). Context units are measured so that the dependencies crossing the scope boundary are complete. It writes:

- `<dir>/evidence.json`, in the format of [references/format.md](references/format.md);
- `<dir>/measured.c4`, the same graph as a LikeC4 model (system `measured`), rewritten on every run. Never edit it.

## Read the exit code first

- **2, no measurement.** The build tool is missing or failed, a scope directory does not exist, or the scope holds no build units. Nothing is written. In a Bazel workspace it never falls back to another build system. Report what failed; do not substitute reading for measuring.
- **1, inconsistent.** The source refers to a build unit its build does not declare (`checks.observed_without_declared`). The code could not build that way, so the measurement is wrong somewhere. Resolve it before relying on the dependencies.
- **0, measured.** Still read the gaps below.

## What the evidence does not show

Name these wherever the evidence is used:

- the global `not_measured` list and each unit's own `not_measured` (generated or missing sources, `select()` branches);
- each unit's `unresolved_deps`;
- `checks.declared_without_observed`: declared, but no reference was found in what was read.

Absence of a dependency in the evidence is established only for what was read. Never report "A does not depend on B" beyond that.

## Languages

| Language | Build units from | Source units and references |
|---|---|---|
| Rust | Bazel (`rust_library`, `rust_binary`, `rust_proc_macro`); Cargo manifests when there is no Bazel workspace | modules by file layout; `use` trees and qualified paths |

A language is added in `scripts/evidence/` as one analyzer that yields units and dependencies in the common format. The format, the graph checks and the LikeC4 output are shared. Bazel reading (`evidence/bazel.py`) is shared by every language built with Bazel.

## Tests

```bash
python3 -m unittest discover -s <skill>/tests
```
