# `composite/v1`: bounded mailboxes and typed ports (`l2r.async/v1`)

`composite/v1` is a new profile version beside `composite/v0`, which is
unchanged and still generates exactly what it did. It exists because three things
the original semantics left open are properties of the *architecture*, and so
must be stated by it, in the semantics, and not decided inside generated Rust:

1. how many messages a mailbox holds, and what happens when it is full;
2. what the environment can structurally send to the system;
3. what the scheduler reports when it cannot make progress.

The semantics is `lean/L2R/Async1.lean`; the worked example is
[`examples/crossing_v1`](../examples/crossing_v1) (a repaired and bounded variant
of `examples/crossing`, which keeps its deliberately failing liveness proof).

## The decisions

| Concern | Decision | Where it is stated or checked |
| --- | --- | --- |
| Capacity | Every child states `mailbox_capacity`. It is a field of the Lean `Network`. | `Network.cap`; derived into `L2RDerived/*.lean` from the model |
| Overflow | **Backpressure, never loss.** A delivery is enabled only if every mailbox that receives one of its outputs has room for all of them; otherwise nothing changes and the sender is blocked. | `deliver`, `fits`; `deliver_bounded`, `Reachable.bounded` |
| Boundary | A port send is permitted only if the target has room. The generated API returns `Err(Rejected)` and the caller keeps the message. | `StepL.send`; `RoundRobin.send_rejected_iff` |
| Blocking | A configuration with pending messages and no enabled delivery is `Blocked`. The scheduler reports it (`Progress::Blocked`) instead of `Settled`. Whether it is reachable is a property to prove. | `Blocked`; `stepOnce_blocked`; for the example `no_deadlock`, `scheduler_never_blocks` |
| Interface | The environment reaches children only through typed ports. A port has a Lean message type and a **total** translation to its target's input. | `Network.ports/Msg/target/inject`; validator rules P001–P004, L006 |
| Admissibility | When the environment may send is a predicate on the current configuration, `Env`, kept apart from the interface. The structural contract is the port types; the temporal contract is `Env`. | `Env`, `anyEnv`; the example's safety theorems hold for **every** `Env` |
| No silent discard | A machine whose outputs nobody consumes is rejected (M007); there are no output ports yet. A translation returning `none` is the Lean specification's own visible decision. | validator |
| Fairness | Not part of the semantics. A liveness theorem takes explicit hypotheses over traces. | `Trace`, `WeaklyFair`, `Recurs`, `fair_termination` |

## The generated code

Mailboxes are `[Option<Input>; N]` plus a length: no allocation, no loops, no
`VecDeque`. `step_once` is straight-line code (one block per round-robin offset).
That shape is chosen for proof: Charon and Aeneas extract it and Lean compiles the
result (see below).

## What is established, and how

| Claim | Kind | Evidence |
| --- | --- | --- |
| The semantics never drops a message for lack of room; no reachable configuration exceeds a capacity | **Proved** (generic, any network) | `deliver_bounded`, `step_bounded`, `Reachable.bounded` |
| Every run of the Lean round-robin scheduler is a permitted behaviour | **Proved** (generic) | `RoundRobin.run_sound`, `send_sound` |
| The scheduler delivers whenever some delivery is enabled; `Settled` and `Blocked` are exact | **Proved** (generic) | `stepOnce_delivered_iff`, `stepOnce_settled_iff`, `stepOnce_blocked` |
| Example: at most one request in the system, latch pending exactly while one is | **Proved**, for every environment | `coherent_of_reachable` |
| Example: capacities respected, no deadlock, the scheduler never reports `Blocked` | **Proved**, for every environment | `mailboxes_bounded`, `no_deadlock`, `scheduler_never_blocks` |
| Example: a pending request is never lost for good (some continuation acknowledges it) | **Proved** — *non-stranding*, **not** liveness | `no_stranding` |
| Example: a pending request is acknowledged on every trace that is **weakly fair** and in which timer expiries are **accepted** again and again | **Proved**, for this example at capacities (2, 2) only (finite abstraction, kernel `decide`) | `CrossingV1Liveness.pending_request_acknowledged` (instance of `fair_termination`) |
| …those hypotheses are satisfiable, and each is needed | **Proved** (explicit traces) | `hypotheses_satisfiable`, `fairness_needed`, `starvation_possible`, `quiescent_pending`, `no_dead_end` |
| …the same, by exhaustive search | **Tested** (cross-check; weaker than the proof, counts only accepted timer sends) | `FairCycles.lean` |
| The round-robin scheduler *provides* weak fairness | **Not proved; assumed.** Only work conservation is proved (`stepOnce_delivered_iff`); bounded waiting of a continuously enabled child is not | — |
| The generated Rust scheduler refines the Lean scheduler model (`new`, `step_once`, both sends; no panic under an invariant that every run preserves) | **Refinement-proved** for the crossing example (2 children, capacity 2); the Lean-side bridge is a kernel `decide` over 1,176 states and does **not** scale to larger networks | `verification/aeneas_v1` (`step_once_refines`, `rustRun_simulates`) |
| …bounded conformance of the same link, as an independent check | **Tested** (all sequences of 10 operations, incl. mailbox contents) | gate 6 of `composition_v1` |
| `run`, `is_settled`, `len`, `get`, and any code that builds a `System` literal violating the invariant | **Outside the refinement claim** | — |
| Real timers, real I/O, platform behaviour | **External assumption** | — |

The finite-abstraction method (`CrossingV1Properties.lean`): a configuration is
abstracted to its two machine states and two mailbox contents; a finite set
`reach` is shown, by kernel `decide` (no `native_decide`), to contain the initial
configuration and be closed under the abstract steps; a simulation lemma shows
every permitted step is an abstract step. Properties are then decided over
`reach`. For `no_stranding` a reverse simulation turns an abstract path into a
real one, with a distance certificate.

## Verification route: what was learned about the tools

- **Aeneas/Charon.** The generated `composite/v1` for the crossing extracts
  (2,159 lines of Lean) and compiles against the pinned Aeneas library, including
  array-backed mailboxes and the unrolled scheduler. The earlier `composite/v0`
  scheduler (`VecDeque`, a `for` loop over a range iterator) was not attempted.
- **How the refinement is proved.** `Result` in this Aeneas release is an
  interaction tree, so `rfl`/`decide` on `step_once` do not work, but specs written as
  `f x ⦃ r => … ⦄` and discharged with `step` do: mailbox push/pop/peek, then each
  `attempt_*`, then `step_once`, then the sends (`verification/aeneas_v1`). The Lean
  side bridges the dependent `Network` to concrete data by a kernel `decide` over
  every well-formed state; that enumeration grows roughly like (2^(c+1)−1)²·24 in
  the capacity c and is specific to this network, so larger networks need a
  symbolic `deliver` lemma. The profile's generated shape is not the obstacle.
- **Verus.** Not added. The Rust-local obligations that remain (array indices in
  range, `len ≤ N`, `next < n`, no overflow) are exactly what an Aeneas
  refinement proof discharges as side conditions of `Result.ok`, so Verus would
  add a second toolchain for the same facts. It stays the candidate if
  panic-freedom of implementer-owned code beyond the machines becomes the
  bottleneck. Verus specifications must never restate `step`.

## Limits

- Output ports (machines whose outputs leave the composite) are not in v1.
- One composite per model, as the root, whose children are machines.
- Failure semantics (loss, reordering, crashes, time) are outside the model.
- Liveness is proved only under stated hypotheses (see `CrossingV1Liveness.lean`):
  weak fairness of deliveries, and timer expiries *accepted* again and again.
  "Accepted" is not a pure environment property: with bounded mailboxes it depends
  on the system having room. A timer that fires periodically but is always
  rejected does **not** satisfy it, and weak fairness does not prevent that run
  (`starvation_possible`: a flood of presses keeps the latch's mailbox full whenever
  the signal could acknowledge). Strong fairness of deliveries would exclude it;
  a round-robin scheduler driven by a caller that interleaves presses does not
  provide strong fairness. Whether to bound presses, require strong fairness, or
  change the design is an owner decision; nothing here chooses.
- The infinite-trace theorem says nothing about finite executions. A request can
  be outstanding in a quiescent system that only the environment can move
  (`quiescent_pending`); every reachable configuration has a successor
  (`no_dead_end`), so no finite prefix is silently excluded for lack of an
  extension.
