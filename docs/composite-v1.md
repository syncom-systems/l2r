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
| Example: acknowledgement happens on every fair run | **Tested, not proved**: exhaustive finite-state search finds a fair pending cycle in the original machines and none in the repaired ones, at several capacities | `verification/composition_v1/FairCycles.lean` |
| The generated Rust scheduler behaves as the Lean scheduler model | **Tested, not proved**: every sequence of 10 operations (sends, `step_once`) agrees on results, states **and both mailboxes' contents** | gate 6 of the lane |
| The generated Rust scheduler is Aeneas-translatable | **Demonstrated** (spike), **not proved** — see below | not in CI |
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
- **What blocks the refinement proof today.** `Result` in this Aeneas release is an
  interaction tree, not an inductive type: `step_once s = ok r` does not hold by
  `rfl` for straight-line code containing binds, and kernel `decide` cannot
  evaluate `Result.match`. A proof therefore needs Aeneas's `simp`/`progress`
  style lemma-by-lemma (mailbox `push`/`pop`/`peek`, then each `attempt_*`, then
  `step_once`) rather than an enumeration. That is the next step; it is not done.
  Until then the Rust ↔ Lean-model link is bounded conformance testing.
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
- Liveness under fairness is not proved (see above). Weak fairness alone is not
  enough here: with bounded mailboxes a delivery can be enabled only
  intermittently, so the example's liveness argument also needs the timer to keep
  being sent, and relies on mailbox bounds to force deliveries.
