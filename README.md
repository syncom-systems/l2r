# L2R (Lean to Rust)

**Status:** early, working vertical slice. ADR-0001 and the follow-up ADRs are still *Proposed*.

L2R is a human-directed architecture modeling and scaffolding tool. A **human
architect** describes the components, their nesting, state ownership and
permitted messages in a versioned model. **Lean 4** owns the behavior. L2R
validates the model against the compiled Lean declarations, shows exactly which
Rust files it would write, and writes them only after the architect approves
that exact plan. Coding agents then implement inside the generated contracts,
in files that L2R never overwrites.

L2R is not a Lean-to-Rust compiler and does not design architectures. Passing
Rust tests is never presented as proof of correspondence with Lean.

## What works today

- **Typed, versioned model** (`l2r.architecture/v0`, TOML). It has explicit ids,
  a component hierarchy, responsibilities, a single owner for each state,
  message edges, Rust mappings, implementer notes and size budgets. Unknown
  fields are rejected.
- **Validation** with 37 stable rule codes, all findings reported at once. Every
  Lean reference must exist in an inventory exported from *compiled* Lean, with
  the right kind and exact type.
- **Deterministic plan.** Every planned file has a path, an ownership class
  (generated or implementation), source components, Lean references and exact
  content. One digest covers the model, the Lean inventory, the generator and
  the profiles.
- **Preview and approval.** The preview shows the effect on disk and the content
  or diff of every change. An approval record must name the digest that was
  reviewed, and a stale record is refused with the reasons. A record is
  evidence of a reviewed plan, not authorization; who may record approvals is a
  repository-governance decision (see [`docs/governance.md`](docs/governance.md)).
  The record also names each existing generated file the plan would replace or
  delete, with its content digest. Nothing is destroyed that it does not name;
  the lock in the output tree is evidence, never authority.
- **A hardened write boundary.** Every path is validated, including paths
  read back from L2R's own state files. The tree is accessed only through
  descriptor-relative, no-follow system calls. Each file change is atomic, and
  an interrupted generation is finished by `l2r recover`. Implementation files
  are never replaced. The exact guarantees and their limits are in
  [`docs/generation-boundary.md`](docs/generation-boundary.md); each claim is
  backed by negative and fault-injection tests.
- **Drift checks.** `l2r check` reports drift, a stale approval, a pending
  journal and budget overruns.
- **Profiles.** `transition-core/v0` is a pure state machine. `composite/v0` is
  deterministic asynchronous composition with per-child mailboxes.
  `crate-root/v0` produces a Bazel package. `composite/v1` (model schema `v1`)
  adds explicit mailbox capacities with backpressure (nothing is dropped), typed
  input ports with explicit rejection, and a scheduler that reports `Blocked`
  instead of stalling silently; see [`docs/composite-v1.md`](docs/composite-v1.md).
- **A reproducible proof lane in CI.** Pinned Lean, Charon and Aeneas run under
  Bazel. The committed Rust implementations of both example machines and both
  message routes are re-extracted and **proved** in Lean to refine their
  specifications, and the proofs are audited (axioms, pinned statements). Seeded
  defects and proof bypasses must be rejected. See
  [`verification/aeneas`](verification/aeneas/README.md) for exactly what this
  covers and what it does not.
- **System properties over every interleaving.** `lean/L2R/Async.lean` defines
  which behaviours an asynchronous composition permits (`l2r.async/v0`).
  `l2r lean-composition` derives each composite's network from the approved
  model; it references the Lean `step` and translation functions and restates
  nothing. For the example, a cross-component safety invariant is proved over
  all permitted interleavings and carried over to a Lean model of the generated
  scheduler. A liveness counterexample is proved too: a pedestrian request can
  be stranded forever. The Lean scheduler model is tied to the generated Rust by
  bounded conformance testing, not proof. See
  [`verification/composition`](verification/composition/README.md).

## Try it

Bazel is the only build and test interface (see ADR-0002). There is no Cargo build.

```sh
bazel test //...                       # all tests, including the example's drift test
bazel test --config=verify //...       # the proof lane (fetches the pinned toolchain, ~8 GB)
bazel build --config=lint //...        # clippy, warnings are errors
bazel build --config=fmt //...         # rustfmt check

bazel build //crates/l2r_cli:l2r
L2R=bazel-bin/crates/l2r_cli/l2r
M=examples/crossing/architecture.toml

$L2R validate $M
$L2R render   $M                       # Mermaid view: containment, ownership, edges
$L2R preview  $M                       # plan, effect on disk, every change, the digest
$L2R approve  $M --digest <digest from preview> --by "<your name>"
$L2R generate $M                       # refuses unless the approval matches the current plan
$L2R recover  $M                       # only after an interrupted generate
$L2R check    $M                       # drift, staleness, budgets
$L2R lean-composition $M               # derived Lean network for the composition proofs
```

Edit a responsibility in the model and run `generate` again. It refuses, because
the architecture model changed, until the new plan has been previewed and approved.

## Layout

| Path | Contents |
| --- | --- |
| `crates/l2r_model` | Model and Lean inventory types, canonical form, digests |
| `crates/l2r_validate` | Validation rules; produces the only input the planner accepts |
| `crates/l2r_plan` | Pure deterministic planner and generation profiles |
| `crates/l2r_apply` | Approval, assessment of disk actions, safe writing, drift checks |
| `crates/l2r_cli` | The `l2r` command |
| `tools/lean_inventory` | Lean exporter for the declaration inventory |
| `examples/crossing` | Generic example: Lean spec, model, approval, generated and implemented scaffold. Its liveness defect is kept on purpose |
| `examples/crossing_v1` | The same system repaired and bounded, for `composite/v1`: typed ports, capacities, system proofs |
| `verification/aeneas` | Rust-to-Lean refinement experiment and its evidence |
| `lean/L2R/Async.lean` | Generic semantics of asynchronous composition and the `composite/v0` scheduler model |
| `examples/crossing/composition` | Derived network of the example and its system proofs |
| `verification/composition` | The composition lane: audit, kernel replay, deviations, conformance |
| `verification/composition_v1` | The same lane for `composite/v1`, with mailbox-content conformance and a fair-cycle search |
| `lean/L2R/Async1.lean` | Semantics `l2r.async/v1`: bounded mailboxes, backpressure, typed ports, fair-termination principle |
| `docs/model-reference.md` | Model schema, profiles, validation rules, write semantics |
| `docs/generation-boundary.md` | Exact write guarantees, threat model, limits, and the tests behind them |
| `docs/verification.md` | Kinds of evidence, the verification route, and what is and is not proved |

## Documentation and decisions

PRDs and ADRs live in [GitHub Discussions](https://github.com/syncom-systems/l2r/discussions):

- [Product Requirements Document (PRD v0.1)](https://github.com/syncom-systems/l2r/discussions/1)
- [ADR-0001: Foundational architecture](https://github.com/syncom-systems/l2r/discussions/2) — Proposed
- [ADR-0002: Bazel is the only build and test interface](https://github.com/syncom-systems/l2r/discussions/4) — Proposed
- [ADR-0003: Architecture model v0, Lean inventory references, and approval semantics](https://github.com/syncom-systems/l2r/discussions/5) — Proposed
- [ADR-0004: Asynchronous composition semantics and the scheduler relation](https://github.com/syncom-systems/l2r/discussions/9) — Proposed

## Known limits

- The Aeneas lane covers the transition functions and routes. The composite
  scheduler is connected to the composition proofs only through a Lean model of
  it and bounded conformance testing, and the system theorems assume an
  environment that the generated API does not enforce. mathlib's compiled
  libraries are trusted inputs (not hash-pinned; their proofs are not
  re-checked, while the repository's own declarations are re-checked by the
  kernel with `leanchecker`). The lane is pinned for Linux x86_64 and macOS arm64 only.
- v0 state types must be field-less Lean inductives. Message edges connect
  siblings only (an open decision in ADR-0003).
- The composite scheduler realizes one deterministic interleaving. It is not a
  model of every interleaving; its runs are proved to be among them only for
  its Lean model.
- The example's composition has a proved liveness defect (a stranded request),
  left unfixed on purpose: it shows what per-machine proofs cannot find.
  `examples/crossing_v1` repairs it at the specification level and proves safety,
  capacity, deadlock-freedom and *non-stranding* (an acknowledgement stays
  possible). Liveness is proved only **under stated hypotheses** (weak fairness of
  deliveries, and timer expiries *accepted* again and again), for the example at
  capacities (2, 2); a periodic but always-rejected timer is a proved
  counterexample under weak fairness alone. The generated `composite/v1` scheduler is tied
  to its Lean model by conformance testing and, for the example, by an Aeneas refinement proof (done for the
  example in `verification/aeneas_v1`; the technique does not yet scale to larger
  networks, see `docs/composite-v1.md`).
- No graphical editor; views are the CLI preview and the Mermaid rendering.

## License

The repository contains an MIT [`LICENSE`](LICENSE), added in its initial
commit. The PRD still lists license selection as a pending maintainer decision.
Confirming the license is that decision; this README does not make it.
