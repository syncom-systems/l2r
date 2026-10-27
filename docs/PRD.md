# L2R — Product Requirements Document

- **Product:** L2R (Lean to Rust)
- **Version:** 0.1 — founding draft
- **Date:** 2026-10-27
- **Status:** Draft for architecture and product review (not an implementation acceptance record)
- **First pilot:** Door Access controller state-machine architecture
- **Core implementation language:** Rust
- **Repository:** https://github.com/syncom-systems/l2r (public)
- **Companion decision:** [ADR-0001](adr/0001-foundational-architecture.md)

## 1. Executive summary

L2R is a human-directed, Rust-based architecture modeling and software scaffolding system. It connects Lean 4 behavioral definitions to an explicitly reviewed architectural graph, produces deterministic Rust contracts, interfaces, test scaffolds, and implementation stubs, and provides traceability for coding-agent implementations and subsequent verification.

**The human owns the architecture.** A coding agent must not decide the structure or silently rewrite formal contracts. The architect needs to see and edit components, ownership, interfaces, dependencies, responsibilities, source mappings, and the *exact files that would be generated* before approving any write.

The intended long-term cycle is:

1. Specify executable and abstract behavior in Lean 4.
2. Model and approve component structure and interfaces.
3. Deterministically generate a Rust project scaffold.
4. Let agents implement within designated ownership boundaries.
5. Translate supported Rust transition logic back into Lean (e.g., using hax, Charon, Aeneas).
6. Prove specified correspondence/refinement and report assumptions and gaps.
7. Explore selected asynchronous interleavings with Stateright; test actual hardware and runtime separately.

This is **not** a general-purpose Lean-to-Rust compiler, an autonomous architecture designer, or a claim that a proof of the Lean specification proves the production Rust automatically.

## 2. Problem and motivation

Architecture decisions are commonly fragmented across conversations, prose, diagrams, tests, code, and formal models. Coding agents can generate large amounts of implementation quickly, but may infer unintended module boundaries, state owners, or interfaces. A separately maintained Lean model can be mathematically sound while the deployed Rust differs from it.

L2R addresses four gaps:

- **Architectural authority:** the human rather than the agent decides responsibilities, state ownership, interfaces, module boundaries and component relationships.
- **Inspectability:** a useful graph, inspector, file tree and plan diff reveal the proposed structure before writing code.
- **Reproducibility:** identical canonical input, pinned profiles and generator version yield byte-identical plans and generated outputs.
- **Verification correspondence:** behavioral and architectural source IDs trace through generated contracts, Rust implementation, Lean translation, tests and explicit proof obligations.

The first consumer is Door Access. Its own Rust OSDP protocol-library implementation continues in parallel and must remain independently buildable and testable.

## 3. Goals and non-goals

### Goals

**G1 — Lean-first behavior.** Lean 4 is authoritative for states, events, transitions, guards, effects, invariants, and relevant cross-machine relations. Support executable transitions as well as abstract nondeterministic composition models.

**G2 — Versioned architecture.** A stable, typed, machine-readable graph defines components, containment, exclusive state ownership, interfaces, message/dependency edges, responsibilities, and Rust crate/module mappings.

**G3 — Human approval.** The architect can examine and revise a proposed graph, annotations, responsibilities, file tree, and complete generation plan. Approval binds to the exact canonical input and planned output digest; changed inputs invalidate it.

**G4 — Deterministic generation.** Produce compilable Rust-facing scaffolds through explicit, versioned profiles rather than LLM-driven file decisions or hidden heuristics.

**G5 — Protected implementation boundaries.** Generated contracts, agent-owned implementations, and human-owned rationale have separate lifecycles. Regeneration never silently destroys hand-written or agent-written work.

**G6 — Verifiable correspondence.** Establish scoped Rust-to-Lean extraction and refinement proofs where supported, report what remains unproved, and detect intentional invariant violations.

**G7 — Concurrency.** Model independently owned machines and asynchronous messaging, and use Stateright to explore relevant interleavings. Use Loom selectively for concrete shared-memory synchronization.

**G8 — Reuse and self-application.** Keep the core generic, employ Door Access as a sanitized pilot, and apply the same deterministic-model and verification practices to L2R itself.

### Non-goals of initial releases

- A complete UML/CASE system or general Lean parser.
- Automatic translation of arbitrary Lean definitions into optimal Rust.
- Automatic proof of arbitrary asynchronous/multithreaded Rust.
- A required full graphical editor before the CLI model and planner work.
- Writing operational Door Access/OSDP code inside the generator repository.
- A coding agent making unapproved architectural changes.

## 4. Roles and user journeys

### Architect

The architect defines or imports Lean declaration references, creates a component hierarchy, chooses state owners and interfaces, sets implementation responsibilities, inspects graphical and source-tree views, adds version-controlled notes, and reviews the deterministic plan and precise file changes before approval.

### Coding agent

The agent consumes a bounded work packet containing approved contracts, interfaces, referenced Lean obligations, tests and implementation stubs. It changes only authorized implementation files or extension points, and reports contradictions or missing assumptions instead of weakening the specification.

### Reviewer / verifier

The reviewer inspects diffs, provenance, approved plan digests, Rust checks, Lean proofs, extraction/refinement results, model checking, and any remaining gaps. A build passing is never represented as proof of behavioral correctness.

## 5. Information model: two linked authorities

### 5.1 Lean behavioral source

Lean owns behavioral declarations: states, events, transition functions/relations, preconditions, error outcomes, effects and invariants. Lean may also describe global asynchronous states with messages, time/deadlines and failures.

The generator initially uses **explicit qualified Lean declaration references and validated metadata**; it does not infer all architecture from arbitrary Lean syntax. A restricted Lean projection or DSL may be introduced later after experiment.

### 5.2 Structural architectural source

A versioned, canonical manifest/graph owns component identities, hierarchy, responsibilities, state ownership, interfaces, permitted communication/dependency edges, Rust crate/module mappings, generation profiles, and human notes. Each fact has one authoritative owner; diagrams are projections of the model.

Minimum model concepts:

- Stable component ID, kind, display name, responsibility and optional notes.
- Parent/child containment and typed edges (ownership, message, dependency, interface).
- State owner and permitted communication boundaries.
- Named interface/event definitions and references to qualified Lean declarations.
- Rust target package, module path and file-profile selection.
- Source provenance, schema/profile versions, approval metadata.

Validation must reject duplicate IDs, dangling references, illegal ownership, forbidden dependency relationships, invalid Rust identifiers/paths, conflicting output paths, and unsupported profile versions.

### 5.3 Consistency and traceability

Lean and architecture references must be machine-checkable as the toolchain permits. No unrelated editable copy of a behavioral fact may live in a UI diagram. Every planned file and implementation contract maps back to a stable component and relevant Lean references.

## 6. Human-centered interaction

L2R needs three synchronized views:

1. **Architecture graph:** nested components, ownership, interfaces, dependency and message edges, with selection and editing.
2. **State-machine view:** linked Lean-defined states, transitions, guards and invariants, showing which definitions are executable, abstract, proved, or draft.
3. **Generated-source view:** exact crate/module/file tree and individual planned file contents, including proposed additions, edits and conflicts.

The component inspector must edit responsibility, constraints, interfaces, Lean mappings, generation profile and human implementation instructions through validated operations. Graphical comments must persist in version-controlled model data.

Early milestones may render Mermaid or another static graph and expose edits through a CLI/editor. A richer browser-based canvas (React Flow) or Rust-native UI (egui) can follow. A diagram is **not** itself an approved architectural revision.

## 7. Generation and approval semantics

**Inputs:** canonical architecture model; relevant Lean declaration inventory; pinned generation profiles/templates; generator version; Rust target restrictions; selected package paths.

**Outputs:** an ordered plan with each file's path, purpose, source-component IDs, ownership class, exact contents/content digest, and diagnostic results.

Required properties:

- Stable deterministic ordering; no timestamps, random IDs, machine-specific absolute paths or ambient iteration order affecting output.
- Dry-run planning and complete file-by-file diff before writing.
- Plan digest that binds approval to specific validated input and output.
- Fail-closed refusal on invalid inputs, stale approval, path traversal, symlink escapes, unowned modification or name conflicts.
- Repeated generation on identical approved input yields identical generated bytes.
- Explicit conflict/migration plan when architecture changes affect agent-owned files.

### File ownership

- **Generated contract:** L2R-controlled; regenerate only from the model/profile; agents must not patch it to mask a contract mismatch.
- **Agent-owned implementation:** may be completed by coding agents; never silently overwritten during regeneration.
- **Human-owned rationale:** review comments, ADRs and architectural explanations managed by human review.

Implementation agents may propose changes to formal behavior or architecture but must not apply unapproved changes, remove failing tests, introduce Lean `sorry`/unreviewed axioms, or weaken theorem statements to pass CI.

Approval records in Git are an auditable development record, not by themselves cryptographic identity or strong organizational authorization. Protected branches and review rules may strengthen them.

## 8. Verification claims and boundaries

Different obligations must be independently reported:

1. **Specification proof:** Lean proves properties of the modeled machine or composition.
2. **Planner/generator validation:** graph consistency, determinism, source traceability, safe writes and file ownership.
3. **Implementation refinement:** supported implemented Rust transition functions are translated into Lean and shown equivalent to or refining the original behavioral specification, including rejection and effects.
4. **Concurrency exploration:** Stateright checks modeled asynchronous actors, message ordering, timeouts and failures within the explored model; Loom may check selected real Rust synchronization primitives.
5. **Integration/hardware tests:** external effects, serial transport, timers, drivers and actuator behavior remain independent test obligations.

A successful Lean build, successful extraction, or proof about an independent model **does not imply** proven correctness of production Rust. Formal reports must list theorem scope, assumptions, model bounds, unsupported constructs, tools/versions, accepted evidence and open obligations.

At least one deliberate invalid Rust transition must be detected by the claimed implementation-verification mechanism before L2R advertises that capability.

## 9. Performance, quality and security constraints

- **Small and comprehensible core:** begin with explicit types and simple functions, avoiding a general modeling language or arbitrary plugin execution.
- **Portable CLI:** planning and generation operate offline and in CI without requiring a hosted service.
- **Reproducible builds:** relevant versions and input digests are pinned for accepted plans.
- **Input safety:** all graph data, paths, names, templates and notes are untrusted; guard against traversal, symlink escape, accidental executable shell hooks and leakage of secrets.
- **Provenance:** planned output identifies its source model, profile/tool version and architectural responsibilities.
- **Human authority:** architecture changes require visible diffs and explicit reapproval.
- **Public examples:** sanitized fixtures only, never Door Access production secrets or proprietary device details.

## 10. Delivery plan and exit criteria

### Increment 0 — Public bootstrap

Repository with README, this PRD, proposed ADR-0001, minimal Rust CLI crate, and initial Rust CI. No claim of functioning generator.

### Increment 1 — Typed model and deterministic planner

Versioned schema, stable IDs, typed graph, validations, profile registry, canonical serialization, deterministic file plan, reproducible digest and human-readable dry run.

*Exit:* two clean runs of identical input produce byte-identical plans; invalid ownership and path-conflict fixtures are rejected.

### Increment 2 — Human review workflow

Graph visualization, file tree, component inspector (initially CLI/editor acceptable), annotations, exact preview and plan-digest approval.

*Exit:* a model edit invalidates approval; the user sees exact diff and must approve before writes.

### Increment 3 — First real Rust scaffold

Generate one approved component workspace with compile/test contracts, traceability, and protected agent-owned implementation files using a sanitized Door Access-style example.

*Exit:* an agent can implement a designated component without inventing an architecture; repeated generation preserves its work.

### Increment 4 — Formal closure experiment

Use compatible pinned hax/Charon/Aeneas tools to translate supported Rust transition logic into Lean and prove a scoped correspondence property. Introduce a defect and demonstrate detection. Explore relevant concurrent event orders through Stateright.

*Exit:* proof results distinguish actual refinement from assumptions, tests and unverified areas.

## 11. Project success criteria

L2R has succeeded at its first useful milestone when a human architect can inspect and approve every component boundary and exact planned Rust file, an agent can implement inside the approved scaffold without changing architecture, regeneration does not destroy agent work, and a specific security-relevant transition property of that implementation is formally checked against its Lean source.

## 12. Risks and open decisions

| Question | Current position |
| --- | --- |
| Can arbitrary Lean be interpreted as architecture? | No; use explicit Lean references and a restricted model first. |
| Can Lean automatically produce executable Rust? | Possible for subsets, but not a prerequisite for deterministic scaffolding. |
| How do we prevent manifest/spec drift? | One owner per fact; stable references, version pins, validation and traceability. |
| Is Stateright a runtime replacement? | No; development-time model checker initially. |
| Which manifest format? | Defer TOML/JSON/Lean-backed syntax choice until a small fixture is proven. |
| Which visual editor? | Evaluate React Flow and egui after the typed model works. |
| Which generation profiles? | Start with a pure transition core, single-writer actor boundary, and transport adapter; ratify profiles incrementally. |
| What license? | Public repository; license selection pending maintainer decision. |
| How are approvals secured? | Git-reviewed digest now, organizational controls later as needed. |
| Which tool versions? | Pin after proof-of-concept compatibility testing. |

## 13. Immediate next actions

1. Review and ratify/revise ADR-0001; keep its status **Proposed** meanwhile.
2. Agree on license policy and branch-protection/maintainer workflow for the public repository.
3. Define the smallest model fixture: parent plus two children, one exclusive state owner, one event edge, one qualified Lean reference and one Rust module mapping.
4. Implement pure graph validation and failure fixtures.
5. Implement canonical deterministic planning before any file writes or GUI.
6. Add exact plan preview, file ownership, and explicit approval.
7. Keep ongoing Door Access OSDP development independent.

**Founding acceptance question:** Is every behavioral fact, architectural decision, generated file and agent implementation assigned one authoritative owner, with a machine-checkable relationship to its source?
