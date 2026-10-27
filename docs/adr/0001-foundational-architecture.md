# ADR-0001: Lean-first behavior, human-owned architecture, Rust-based deterministic scaffolding

- **Status:** Proposed — requires maintainer ratification
- **Date:** 2026-10-27
- **Decision owners:** L2R project maintainers / architects
- **Related:** [PRD v0.1](../PRD.md)
- **Scope:** Foundational design and trust boundaries, not final technology selection

## Context

L2R aims to turn a formally described software design into a predictable, implementation-ready Rust architecture without delegating architecture choices to coding agents. The first pilot will be the Door Access controller, where hierarchical state-machine architecture is being analyzed in parallel with independently ongoing Rust OSDP library development.

Traditional architecture descriptions span Markdown, diagrams, code, design discussions and proof models. Agents may implement code well, but they are not given authority to select ownership models, component boundaries and file structures. The human architect must be able to view, edit and explicitly approve the proposed structure and the exact generated file plan.

Lean 4 specifications define behavior, but do not inherently prescribe how crates, modules and files are arranged. Conversely, source layout does not prove a state machine behaves correctly. The model must distinguish these separate concerns while connecting them through verifiable references.

## Decision

### D1 — The generator core is implemented in Rust

Rust owns typed architecture-model handling, graph validation, deterministic planning, file generation, provenance, and safe-write rules. It should permit the same type-safety, state-model and selective verification practices used for generated Rust projects. A user-interface technology may differ but cannot bypass the Rust planner and validators.

### D2 — Lean is authoritative for behavioral semantics

Lean 4 defines device and system states, events, guards, transitions, effects, invariants and relevant cross-machine relations. Both executable functions and abstract nondeterministic relations are supported conceptually. L2R may initially link to named Lean declarations rather than parse or compile arbitrary Lean into Rust; no universal Lean-to-Rust compiler is promised.

### D3 — A versioned architectural graph is authoritative for structure

The architectural model owns stable component IDs, hierarchy, ownership, message and dependency edges, interfaces, responsibilities, agent instructions, Rust package/module mappings, selected generation profiles, and persisted annotations. It is machine-readable and version-controlled. A diagram is a view or editor of this same canonical model, never a detached shadow version.

**Rule:** one authoritative source for each fact. Behavioral truth belongs in Lean; implementation-structural truth belongs in the architectural model. Links must be validated.

### D4 — The architect approves the exact generation plan

Before any write, L2R shows validated architectural structure, resolved state ownership and interfaces, the planned file tree, precise file contents or diffs, and provenance. Approval binds to a specific model/plan revision or digest. Any change affecting the plan invalidates prior approval.

Git approval logs are useful records but are not inherently secure identity proofs; stronger GitHub branch rules and role controls may be applied separately.

### D5 — Deterministic, versioned generation profiles define file structure

A small explicit catalog of profiles maps approved component types to Rust modules, interfaces, contracts, tests and implementation stubs. Candidate examples include a pure transition function, a single-writer actor boundary and a transport adapter. The same canonical input, pinned generator and profiles must generate identical output bytes; no agent makes hidden architectural decisions during generation.

Complete Rust transition generation may be investigated for a controlled, semantics-preserving subset, but is not a prerequisite for the first usable product.

### D6 — Generated contracts and agent-owned code have separate lifecycles

Generated artifacts are modified at their source (Lean or architectural model/profile) and regenerated. Agent-owned and human-owned files are not silently overwritten. Structural changes requiring implementation changes produce a migration/conflict report and a reviewed plan. Agents may propose modifications to the architecture, but may not ratify them or weaken formal requirements.

### D7 — Visual editing is synchronized with the source model

A future editor provides component, state-machine, and generated-file views. Supported graph edits must update the canonical model and pass validation. Diagram layout is presentation metadata. The Rust CLI remains useful and deterministic independently of a graphical interface.

### D8 — Verification requires explicit correspondence, not assertion

A proof about the Lean state machine is not automatically a proof about production Rust. Where supported, hax/Charon/Aeneas may translate implemented Rust transition logic into Lean for an explicit equivalence/refinement argument. Stateright explores modeled asynchronous message/event interleavings; Loom may examine selected shared-memory synchronization. Hardware and external side effects require separate integration verification.

Reports must distinguish proved theorems, verified implementation correspondence, tested cases, modeled bounds, assumptions, unsupported translation, and open obligations. Silent weakening of theorem statements, deletion of failing tests, new unreviewed axioms or Lean `sorry` is forbidden in accepted security proofs.

### D9 — L2R remains independent of its first pilot

Door Access provides an early real-world fixture and integration pilot, but no production OSDP implementation or secrets belong in the core generator. Existing Rust OSDP work must continue independently and is not gated on L2R maturity.

## Rationale

Human-controlled structure addresses the architectural trust problem of coding agents. Lean defines formal behavior, the graph records implementation architecture, deterministic profiles make the resulting file set predictable, and explicit approval supplies an audit point. Separation of generated and agent-owned files makes iterative regeneration safe. Keeping the core Rust-based and CLI-first prevents the project from becoming a heavyweight visual CASE tool before generation semantics are stable.

A restricted and explicit approach is preferable to inventing a general-purpose modeling language or requiring a complete verified Lean-to-Rust compiler at project inception.

## Alternatives considered

| Alternative | Why rejected as the foundation |
| --- | --- |
| Rust-first code with independent after-the-fact Lean model | Risks semantic drift and ambiguous correspondence. |
| Direct general Lean-to-Rust compilation | Unnecessary for scaffolding and difficult to guarantee for arbitrary code. |
| Agent-selected architecture and file layout | Nondeterministic and violates intended human authority. |
| Graphical diagram as sole source | Loses stable identities, full contracts and machine-checkable traceability. |
| Heavy UML/CASE platform | Too much up-front complexity and coupling for the narrow problem. |
| Build a complete GUI before a typed planner | UI would precede canonical model and safe generation semantics. |

## Consequences

**Benefits:** clear architecture ownership, reviewable changes, deterministic file layouts, protected agent work, source traceability, a path to implementation refinement proofs, and general use across Rust projects.

**Costs:** Lean and the structural model require explicit references, validation and versioning. Generation profiles require controlled evolution. An interactive editor has a real maintenance cost. Rust-to-Lean extraction and refinement may only cover selected transition code.

**Risks and controls:**

- Model/specification drift → validate qualified Lean references and source/profile version identities.
- Misleading verification claims → separate proof status from test/extraction status and record assumptions.
- Agent editing generated contracts → file ownership rules, CI drift detection and protected review.
- Destructive generation → full dry-run diff, plan digest, path/symlink checks, and conflict refusal.
- Tool sprawl → defer advanced GUI, universal parsers, direct compilation and unnecessary frameworks.

## Initial implementation boundary

Begin with a minimal versioned architecture schema, a typed graph, pure validation, canonical serialization, deterministic file planning, exact plan preview and a human approval checkpoint. A realistic fixture should model one state-owning component and two event-producing components, with explicit Lean references and Rust module paths.

The first delivery is **scaffolding**, not an automatically verified production state-machine generator.

## Deferred decisions

- Manifest serialization: TOML, JSON, or a constrained Lean-linked DSL.
- Profile catalog beyond initial minimal types.
- Web graph editor (e.g., React Flow) versus Rust-native editor (e.g., egui).
- Lean declaration projection/import strategy and pinned translation-tool versions.
- License and external contribution policy.
- Organizational review rules and approval attestations.
- CI enforcement of refinement once proof coverage is demonstrated.

These should be resolved with separate ADRs or explicit amendments after relevant experiments, rather than silently being assumed now.

## Ratification conditions

Maintainers should accept ADR-0001 only after agreeing that:

1. Human architects retain final authority for structure and ownership.
2. Lean semantics and architecture structure are separate, linked, reviewable authorities.
3. Every generation write requires validation and approval of the exact plan.
4. Generated contracts and agent implementations cannot be silently overwritten.
5. Security-relevant verification claims require supported implementation correspondence and explicit assumptions.
6. L2R is independent of its Door Access pilot.

**Status remains PROPOSED** until these conditions are explicitly reviewed and ratified.
