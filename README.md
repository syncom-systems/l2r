# L2R (Lean to Rust)

**Status:** Public project bootstrap. The product requirements and founding architecture decision are drafts for review.

L2R is a Rust-based, human-directed architecture modeling and scaffolding system. It links Lean 4 behavioral specifications to explicitly approved software architecture and generates deterministic Rust interfaces, contracts, and implementation stubs for coding agents.

The intended verification loop is **Lean specification → approved architectural model → deterministic Rust scaffold → agent implementation → Rust-to-Lean extraction → refinement proof**. Translation support and proof coverage are limited by tool capabilities and assumptions; no automatic proof of arbitrary Rust is claimed.

## Documentation

- [Product Requirements Document (PRD v0.1)](docs/PRD.md)
- [ADR-0001: Foundational architecture (proposed)](docs/adr/0001-foundational-architecture.md)

## Principles

1. Human architects determine components, responsibilities, ownership, interfaces, and implementation boundaries.
2. Lean owns behavioral semantics. A versioned architectural model owns software structure and human instructions.
3. Diagrams are editable views of the model, not competing sources of truth.
4. Planning and generation are deterministic, previewable, and explicitly approved.
5. Regeneration must not overwrite agent-owned implementation files.
6. Agents implement approved contracts and report contradictions rather than weaken specifications or proofs.
7. The first pilot is Door Access, but the generator core stays project-independent.

## Bootstrap

Only a minimal Rust CLI skeleton and planning documents exist. Validation, modeling, generation, graphical editing, and verification integration are planned work, not delivered functionality.

```sh
cargo run -- --help
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## Public repository and license

This repository is public but has **no open-source license selected yet**. Public visibility does not itself grant reuse rights. Selection of a license is a project-maintainer decision.

The ongoing Rust OSDP library implementation in Door Access continues independently; it must not be blocked by L2R development.
