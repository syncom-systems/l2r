# The composition lane: system properties over every permitted interleaving

```sh
bazel test --config=verify //verification/composition:composition_test
l2r lean-composition examples/crossing/architecture.toml   # the derived network
```

Proving each machine and each message translation correct (the Aeneas lane)
says nothing about the system they form. This lane connects four things that
have to agree:

```
 Lean machines (spec)            Crossing.Latch.step, Crossing.Signal.step, Routes.*
        │  referenced by name, never restated
 approved composition            examples/crossing/composition/L2RDerived/crossing.lean
        │  derived from the architecture model by `l2r lean-composition`; freshness-tested
 permitted behaviours            L2R.Async.Step / Reachable  (lean/L2R/Async.lean)
        │  ⊇  proved: RoundRobin.run_sound (every scheduler run is permitted)
 scheduler model                 L2R.Async.RoundRobin  (mirrors the composite/v0 Rust)
        │  bounded conformance testing, not proof: 78,125 operation sequences
 generated Rust scheduler        examples/crossing/scaffold/src/crossing/mod.rs
```

## The semantics (`l2r.async/v0`, `lean/L2R/Async.lean`)

| Concern | v0 decision |
| --- | --- |
| State ownership | Each child owns its state. Only a delivery to that child changes it, through its own `step` (`deliver_some` states it). |
| Message ordering | One FIFO mailbox per receiver. Messages are appended in production order, from any sender and from the environment. |
| Granularity | Delivery is atomic (run to completion): pop one message, `step`, append every translated output in approved-edge order. |
| Interleavings | Any child with a non-empty mailbox may be served next. The environment may send any input it is permitted to, at any time. |
| Environment | `Env`, an explicit parameter of every system theorem. The generated API (`System::send`) accepts anything (`anyInput`). |
| Bounded resources | Mailboxes are unbounded. A bound is a **property** to prove for a system, under its environment (e.g. `at_most_one_request`). |
| Failure | Not modelled in v0: message loss, duplication, reordering, crashes, time. These are omissions, not assumptions. |
| Termination | Not assumed. The scheduler's `run` has a step budget; `stepOnce_none_iff` characterises when it stops. |
| Fairness | Not part of the semantics. Liveness results are stated so that fairness is either irrelevant or an explicit hypothesis. |

The *execution order* of a scheduler and the *set of permitted behaviours* are
kept apart:

- `Step` and `Reachable` define what is permitted.
- `RoundRobin` is one way to choose among the permitted steps.
- `RoundRobin.run_sound` proves that it chooses only permitted ones, which is
  trace inclusion.

The seeded `sched_sticky_cursor` deviation shows why the distinction matters.
It is an unfair, differently ordered scheduler, and every theorem still holds,
because its runs are permitted too. Only conformance testing tells it apart from
the generated Rust.

## What is proved (machine-checked, audited, kernel-replayed)

Generic, once for every network:

| Theorem | Meaning |
| --- | --- |
| `RoundRobin.serve_eq_deliver` | The scheduler's delivery (written to mirror the Rust: `pop_front`, `step`, `push_back` per edge) is exactly the abstract atomic delivery. |
| `RoundRobin.stepOnce_sound`, `run_sound` | Every scheduler step is a permitted step. Every configuration the scheduler reaches, with a caller restricted to `E`, is reachable by permitted behaviour. |
| `RoundRobin.run_invariant` | A system invariant proved over permitted behaviour holds for the scheduler. |
| `RoundRobin.mem_order`, `stepOnce_none_iff` | Progress: the scheduler tries every child and stops only when every mailbox is empty. |
| `reachable_invariant`, `star_invariant`, `deliver_some` | Proof principles for system invariants. |

For the crossing composition, in the **physical** environment (the button sends
presses to the latch, the timer sends expiries to the signal):

| Theorem | Kind | Meaning |
| --- | --- | --- |
| `coherent_of_reachable` | safety | In every reachable configuration, at most one pedestrian request is in the system (in flight to the signal, being served in amber, or acknowledged in flight back). While one is, the latch is pending. |
| `at_most_one_request` | safety, bounded resource | The signal's mailbox never holds more than one request. Timer expiries are not bounded; nothing bounds the environment. |
| `acknowledgement_only_to_pending` | safety | An acknowledgement only ever travels to a pending latch. |
| `no_conflicting_release` | safety | The signal's own invariant, lifted to every configuration. This is the routine case. |
| `scheduler_coherent` | safety, scheduler | The above holds for every run of the round-robin scheduler. |
| `request_can_be_stranded` | **liveness counterexample** | A reachable configuration exists where the latch is pending and **no** permitted continuation ever acknowledges it. `scheduler_strands_a_request` shows that the generated scheduler reaches it. |

`request_can_be_stranded` is a real defect of the example's composition, and no
per-machine proof can find it. `Signal.step` ignores a crossing request outside
the green phase, and the latch coalesces presses while pending. A request that
arrives while the signal is walking is therefore dropped, the latch waits
forever, and every later press is silent. Both machines satisfy their own
theorems. Because *every* continuation is stuck (`stranded_forever`), no
fairness assumption rescues it. The example is **not** fixed in this increment
(see the PR's recommended next step).

## The gates

| Gate | Check | Can fail? (seeded in this lane) |
| --- | --- | --- |
| 1. Prove | The library, the spec, the derived network and the system proofs compile with the pinned Lean. **No mathlib and no Aeneas library.** | — |
| 2. Audit | Every expected theorem exists, uses only `propext`/`Classical.choice`/`Quot.sound`, and has its reviewed statement. The **definitions** behind the statements are pinned as well: semantics, scheduler model, environment, predicates. | `sorry`, `native_decide`, a new axiom, a weakened statement, a weakened definition, `#exit` (`bypasses.tsv`) |
| 3. Kernel | `leanchecker --fresh` replays the proofs and everything they import, **Lean's core library included**, so no compiled `.olean` is trusted. `KernelForgery.lean`, whose proof of `False` skipped the kernel and which uses only standard axioms, must be rejected. | The forgery fixture |
| 4. Deviations | Each seeded change is rejected inside its named theorems, or proves the same statements if `accepted`. Scheduler model: serving the newest message, routing to the front, dropping an output, forgetting the new state. Spec: a signal that acknowledges on returning to green. That change keeps every per-machine theorem but breaks the system invariant. Environment: allowing the environment to inject requests. | `mutants.tsv` |
| 5. Bypasses | See gate 2. | `bypasses.tsv` |
| 6. Conformance | **Testing, not proof.** Every sequence of 7 operations (sends and `step_once`), run on the generated Rust `System` and replayed on the Lean model, gives identical `step_once` results and states. An `accepted` order deviation must disagree. | `sched_sticky_cursor` disagrees on 11,051 of 78,125 sequences |

The derived network's freshness is tested in the ordinary run
(`//crates/l2r_plan:l2r_plan_lean_composition_test`). It must equal
`l2r lean-composition` of the committed model and inventory. Its header records
both digests. Deriving it never changes a generation plan. It is not a
`composite/v0` artifact, so the profile's output is unchanged.

## What is not verified

- **The Rust scheduler is not proved to implement the Lean scheduler model.**
  That link is review of the `composite/v0` profile plus bounded conformance
  testing (gate 6), which also observes only `step_once` results and states, not
  mailbox contents. Proving it needs the scheduler in a form Charon/Aeneas can
  translate (fixed-capacity arrays instead of `VecDeque`, so mailbox bounds
  become explicit). That is a new profile version, not a change to `composite/v0`.
- **The environment contract is not enforced.** The physical-environment
  theorems hold for callers that send only presses to the latch and expiries to
  the signal. The generated API accepts any envelope. The `env_injects_requests`
  deviation shows that the safety result depends on this. Typed ports
  (`composite/v1`) are the intended way for the architecture to state it.
- **Nested composites** are refused by `l2r.lean-composition/v0`. Only
  composites whose children are all machines are derived.
- **Fairness-dependent liveness** is not stated. The scheduler's bounded-waiting
  property (every non-empty mailbox is served within `n` steps) is true of
  round-robin but not yet proved. Only progress is.
- **Failure semantics** (loss, crashes, time) are outside v0.
- **Trusted base:** the Lean kernel (replayed by `leanchecker`, but still the
  same kernel); review that `L2R/Async.lean` states the intended semantics
  (pinned by the audit); review that `RoundRobin` mirrors the profile.

## Why Lean, and not a separate model checker

The behaviour is already in Lean, and the semantics and the system are expressed
in it, so nothing is restated. The derived network only *references* the spec's
`step` functions. A model checker (TLA+, Stateright) would need the machines
re-encoded or a new derived translation. It is attractive for larger state
spaces and for finding counterexamples, and the right form is a generated,
digest-pinned export from the same network, never a hand-written model. For a
system this small, inductive invariants and explicit witnesses in Lean suffice.
