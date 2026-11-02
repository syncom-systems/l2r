# L2R (Lean to Rust)

**Status:** Public project bootstrap. The product requirements and founding architecture decision are drafts for review.

L2R is a Rust-based, human-directed architecture modeling and scaffolding system. It links Lean 4 behavioral specifications to explicitly approved software architecture and generates deterministic Rust interfaces, contracts, and implementation stubs for coding agents.

The intended verification loop is **Lean specification → approved architectural model → deterministic Rust scaffold → agent implementation → Rust-to-Lean extraction → refinement proof**. Translation support and proof coverage are limited by tool capabilities and assumptions; no automatic proof of arbitrary Rust is claimed.

## Documentation

PRDs and ADRs live in [GitHub Discussions](https://github.com/syncom-systems/l2r/discussions):

- [Product Requirements Document (PRD v0.1)](https://github.com/syncom-systems/l2r/discussions/1)
- [ADR-0001: Foundational architecture (proposed)](https://github.com/syncom-systems/l2r/discussions/2)

## Principles

1. Human architects determine components, responsibilities, ownership, interfaces, and implementation boundaries.
2. Lean owns behavioral semantics. A versioned architectural model owns software structure and human instructions.
3. Diagrams are editable views of the model, not competing sources of truth.
4. Planning and generation are deterministic, previewable, and explicitly approved.
5. Regeneration must not overwrite agent-owned implementation files.
6. Agents implement approved contracts and report contradictions rather than weaken specifications or proofs.
7. The core stays independent of the projects that adopt it; examples and fixtures are original and generic.

## Bootstrap

Only a minimal Rust CLI skeleton and planning documents exist. Validation, modeling, generation, graphical editing, and verification integration are planned work, not delivered functionality.

```sh
cargo run -- --help
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## Public repository and license

The repository contains an MIT [`LICENSE`](LICENSE), added in its initial commit. The PRD still lists license selection as a pending maintainer decision; confirming the license is that decision.

Projects that adopt L2R continue their own development independently and are never blocked by L2R's maturity.
