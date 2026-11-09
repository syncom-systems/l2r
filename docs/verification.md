# Verification in L2R: kinds of evidence and the route between them

L2R keeps one authority per fact. **Lean owns behavior.** The architecture model
owns structure. Rust is the implementation. Verification tools produce evidence
*about* these; none of them may become a second place where behavior is defined.

## Five kinds of evidence, never interchangeable

| Kind | Question answered | Tool / artifact | Status in this repository |
| --- | --- | --- | --- |
| **1. Specification proofs** | Does the Lean specification have the properties we want? | Lean theorems in the spec project (e.g. `Crossing.Signal.no_conflicting_release`) | Proved for the example spec; referenced from the model as `invariants` |
| **2. Implementation correspondence** | Does the Rust implementation compute what the Lean specification says? | Charon + Aeneas translate Rust to Lean; a refinement theorem relates the translation to the spec under explicit abstraction functions | Proved in CI for both machines (`request_latch`, `signal`) and both message routes (`verification/aeneas`). **Not** for the composite scheduler |
| **3. Rust-local properties** | Is the implementation free of panics and overflow, and are its data-structure invariants maintained, independently of behavior? | Aeneas (panic freedom follows from the `Result` being `ok`), or Verus/Kani for properties that do not restate behavior | Panic freedom proved for both machines and the routes as part of (2); nothing else yet |
| **4. Composition** | Do the machines, composed asynchronously, satisfy system properties under *all* permitted interleavings, and does the scheduler choose only permitted ones? | The generic semantics `lean/L2R/Async.lean` (`l2r.async/v0`), a network derived from the model by `l2r lean-composition`, and system proofs (`verification/composition`) | Proved for the crossing example: a cross-component safety invariant over every interleaving in a stated environment, its transfer to a Lean model of the `composite/v0` scheduler, and a **liveness counterexample**. `composite/v1` (`lean/L2R/Async1.lean`, `l2r.async/v1`, [`composite-v1.md`](composite-v1.md)) adds capacity, backpressure and typed ports to the semantics and proves safety, capacity, deadlock-freedom and non-stranding of the repaired example for every environment. The Lean scheduler model ↔ generated Rust link is bounded conformance testing (v1 compares mailbox contents too), not proof |
| **5. Runtime and platform evidence** | Does the deployed system behave on real hardware, with real I/O and timing? | Integration tests, hardware-in-the-loop, monitoring | Out of scope for the generator; the example has unit tests only |

A result of one kind never stands in for another. Passing Rust tests is kind 5
evidence for tested inputs. It is not correspondence (2). A proof about the spec
(1) says nothing about the code until (2) exists. A verified transition function
(2) does not verify the composed system (4).

## Scheduler order vs. the space of interleavings

`composite/v0` generates a scheduler that serves children round-robin in
identifier order, with FIFO mailboxes. That is **one** execution order, chosen to
be deterministic and testable. The abstract semantics (`L2R.Async.Step`) admits
every interleaving of atomic deliveries, with FIFO order per mailbox. Therefore:

- System properties are proved over the permitted behaviours (`Reachable`),
  for an explicitly stated environment.
- The scheduler is shown to choose only permitted steps
  (`RoundRobin.run_sound`), so those properties hold for it by inclusion.
- Inclusion does not pin the order. An unfair scheduler is also included.
  Bounded conformance testing ties the Lean scheduler model to the generated
  Rust's actual order.
- Safety and liveness are separate obligations. Safety is an inductive
  invariant over `Step`. Liveness is stated either for *every* continuation
  (needs no fairness) or under an explicit fairness hypothesis. The crossing
  example fails liveness in the strong sense: a pending request can be stranded
  on every continuation.

The details, the gates and the remaining gaps are in
[`verification/composition`](../verification/composition/README.md).

## Kernel replay (`leanchecker`)

Both lanes run `leanchecker`. An axiom audit alone cannot see a declaration that
never reached the kernel. A proof added with `debug.skipKernelTC` compiles and
uses only standard axioms. Each lane keeps such a forgery as a fixture that must
be rejected.

- **Composition lane:** `leanchecker --fresh` replays everything, Lean's core
  library included (about 30 s), because the lane imports no mathlib. No
  compiled `.olean` is trusted.
- **Aeneas lane:** the spec, translation and proof modules are re-checked. The
  imported Aeneas and mathlib declarations are not replayed; a local `--fresh`
  run did not finish within 55 CPU-minutes. They remain trusted compiled inputs.
  A scheduled full replay is the next step.

The toolchain also ships independent kernels (`lean4lean`, and `nanoda` via
`leanexport`). Running a second, independently written kernel over the exported
environment would cover a bug in the Lean kernel itself. It is not done yet.

## Where Verus fits

Verus does not by itself create a competing behavioral authority. That happens
only if Verus specifications *restate* the behavior that Lean owns, for example
by re-encoding the transition table as `ensures` clauses. Used for kind 3
obligations, Verus complements Lean. Examples:

- no panics or arithmetic overflow in implementation code (including code Aeneas
  cannot translate, such as the `VecDeque`-based scheduler);
- bounds and data-structure invariants: mailbox capacity, index validity;
- frame conditions: a step mutates only the owning component's state.

**Evaluation (this increment).** No Verus toolchain was added. Today it would
give little coverage that the lanes lack:

- The machines and routes are pure functions, already proved panic-free by
  Aeneas.
- The `composite/v0` scheduler's local properties are simple. `VecDeque`
  indexing cannot fail, and the `next` cursor is always `< n`.

**Update (`composite/v1`).** The bounded-mailbox scheduler now exists, as
fixed-size arrays with straight-line code, and extracts with Charon/Aeneas (see
[`composite-v1.md`](composite-v1.md)). Its index and capacity facts are the side
conditions an Aeneas refinement proof discharges, so Verus is still not added;
this paragraph is the reasoning that led to adding no second toolchain.

Verus would become worthwhile with a bounded-mailbox scheduler: ring-buffer index
arithmetic, capacity accounting, the absence of overflow on `push`, and frame
conditions per child. These are exactly the Rust-local obligations that the
Lean semantics deliberately leaves to the implementation. Its specifications
must then refer to the Lean-derived capacity bound and never restate `step`.

Rule for any such tool: its specifications may mention only Rust-local facts. If
an obligation needs behavioral meaning, it must be stated in Lean and connected
through correspondence (kind 2). If that rule turns out to be too restrictive
for some property, the right fix is a generated, digest-pinned translation from
Lean, not a hand-written restatement.

## Fairness and liveness are separate, and stated with their assumptions

Three different claims are easy to conflate:

- **Non-stranding**: from every reachable configuration, *some* permitted
  continuation reaches the goal. Proved for the repaired example
  (`no_stranding`); it needs a cooperative environment and says nothing about
  what the scheduler actually does.
- **Liveness under fairness**: *every* run satisfying explicit fairness
  hypotheses reaches the goal. `L2R.Async1.fair_termination` is the proof
  principle (a ranking with helpful actions over a `Trace`, with weak fairness of
  deliveries and a recurring environment event as hypotheses). The example's
  liveness is **not proved**; it is tested by an exhaustive finite-state search
  (`FairCycles.lean`). Weak fairness is not sufficient with bounded mailboxes: a
  delivery can be enabled only intermittently, so the argument also uses that
  the timer keeps being sent and that bounds force deliveries.
- **Unconditional progress**: false whenever the environment can withhold a
  required event forever (here, the timer). L2R does not claim it.

## Traceability: when a proof counts

A correspondence proof is accepted only if all of the following hold. The proof
lane (`bazel test --config=verify //verification/...`) checks each of them in CI.

1. **Source identity.** The translation was produced from the exact committed
   Rust sources. It is regenerated in CI from those files, not from a copy, and
   must equal the committed translation byte for byte.
2. **Spec identity.** The proof imports the same Lean spec that produced the
   inventory that the approved plan is bound to.
3. **Tool identity.** Charon, Aeneas, Lean and the Rust nightly Charon uses are
   pinned to public releases by sha256. mathlib's compiled libraries are fetched
   at a pinned revision by mathlib's cache tool and are trusted, not hash-pinned
   here.
4. **No bypass.** Every expected theorem is present, has its expected statement,
   and depends only on the standard axioms (`propext`, `Classical.choice`,
   `Quot.sound`). `sorryAx`, `Lean.ofReduceBool` (`native_decide`) and
   translation-introduced axioms are rejected. The axiom check alone is not
   enough: a declaration added with `debug.skipKernelTC` compiles and uses only
   standard axioms. Every declaration of the spec, translation and proof is
   therefore re-checked by the kernel with `leanchecker`, and a fixture with a
   kernel-skipping proof must be rejected.
5. **Detection.** Each deliberately wrong implementation (a mutant) makes the
   proof fail inside the named refinement theorem. This shows the check can fail
   at all.
6. **Meaningful statements.** Theorem statements and the abstraction functions'
   defining equations equal a reviewed, committed expectation. A permuted
   abstraction function would otherwise keep every statement's text.

Compiling the translated Lean is not a proof check. Only the theorem statements
and the axiom report are.

## Trusted base (for kind 2)

The Lean kernel; Charon and Aeneas translating Rust faithfully (the translation
is not itself verified); rustc compiling the source as Charon read it; the
abstraction functions, which are part of the reviewed statement. Aeneas's
standard-library models contain `sorry` in some modules (e.g. slices). The axiom
check (4) ensures no accepted proof depends on them.

## Scheduled dependency recheck (separate from the PR checks)

The PR lane replays its own declarations through the kernel but trusts the
imported compiled libraries (mathlib, Aeneas' Lean library and their
dependencies). A weekly workflow, `.github/workflows/dependency-recheck.yml`
(also `workflow_dispatch`), replays those too:

```sh
bazel test --config=verify --test_tag_filters=dependency-recheck --test_timeout=6600 \
  //verification/aeneas:dependency_recheck
```

**The foundation does not depend on this job.** It has no `pull_request`
trigger, is not a required check, is tagged `manual` and `dependency-recheck`
(so `bazel test //...` and `--config=verify //verification/...` never run it),
and no PR waits for it. Its result is a periodic audit of trusted inputs, not a
merge gate.

- **What is replayed.** The import closure of `Refinement`: the lane's modules
  plus every module they transitively import (about 1,740 outside the Lean
  core). `leanchecker <Prefix>` runs once per shard; each module's own
  declarations are re-checked by the kernel on top of its imports, so every
  declaration in the closure is checked once. `leanchecker --fresh` would do
  the same in one single-threaded process and did not finish in 55 CPU-minutes;
  shards run in parallel and are reported one by one. Only closure modules are
  replayed, not all of mathlib.
- **Still trusted.** The Lean kernel and the `lean`, `lake` and `leanchecker`
  binaries (the same executables do the replay; this is not an independent
  checker), the Lean core library (`Init`, `Std`, `Lean`, `Lake`; set
  `RECHECK_CORE=1` to replay it as well), Charon, Aeneas, rustc, the host, and
  the abstraction functions. The replay detects declarations in the compiled
  libraries that bypassed the kernel (for example through
  `debug.skipKernelTC`); it does not make the kernel itself trustworthy.
- **Schedule and bounds.** Sundays 03:17 UTC. `timeout-minutes: 150`, Bazel
  `--test_timeout=6600`, a replay budget of 5400 s (`RECHECK_BUDGET_SECONDS`),
  2 threads per `leanchecker` process (`RECHECK_THREADS`, i.e. `LEAN_NUM_THREADS`)
  and shards of at most 200 modules (`RECHECK_MAX_SHARD`). Change them on the
  `bazel test` line of the workflow (`--test_env=NAME=value`), or per manual run
  with the `budget_seconds` and `threads` inputs. Runs never overlap
  (`concurrency` group `dependency-recheck`, no cancellation). If you raise the
  budget, keep `--test_timeout` and `timeout-minutes` above it.
- **Outcomes.** `complete` passes. `failed` (a shard was rejected or crashed)
  and `incomplete` (the budget ran out, or the job hit its time limit and was
  killed) both fail the run; a timeout is never a pass. A failure opens, or
  comments on, a single issue titled "Scheduled dependency recheck failed"
  (label `dependency-recheck`); the next successful run closes it.
- **Evidence.** The `dependency-recheck-evidence` artifact holds Bazel's test
  outputs (`test.outputs/`, or `outputs.zip` there), containing `dependency-recheck.json` (status and
  reason, toolchain manifest, mathlib and other package revisions from the
  Aeneas `lake-manifest.json`, `leanchecker` sha256, closure size, modules
  checked, wall time, budget, threads, per-shard status/time/exit code, and the
  tail of the first failing shard's log), `modules.tsv` (every closure module
  and its file), `shards.tsv`, and one log per shard. A run killed by the job
  time limit may have no JSON.
- **Expected cost.** The workflow has not run on GitHub yet; these are a local
  probe and an extrapolation. Measured on a 14-core, 48 GB macOS arm64 machine
  that was also busy with an unrelated job (load average about 15), one
  `leanchecker <Prefix>` process per shard: Batteries (77 modules, 4 threads)
  10 s, 3.0 GB peak RSS; Aesop (132, 4 threads) 18 s, 3.7 GB; Aeneas (249, 4
  threads) 99 s, 9.4 GB; `Mathlib.Order` (134, 2 threads) 58 s, 3.7 GB;
  `Mathlib.Tactic` (218, 2 threads) 59 s, 3.7 GB. Each module task builds its own
  environment, so peak RSS (which includes mapped `.olean` files) grows with
  threads, hence the default of 2. Extrapolated, not measured: the whole closure
  (about 1,740 modules) should take on the order of 10 to 30 minutes of wall time
  at 2 threads on a 4-core Linux runner, with 4 to 6 GB per process. The
  uncertainty is large (module cost varies widely, the runner is slower and has
  less memory, and mathlib's `Algebra` and `Data` shards were not measured), so
  the 5400 s budget leaves several-fold headroom. Use the first scheduled runs
  to calibrate the budget, and treat anything but `complete` as not checked.

## Supported subset

Correspondence is practical for the code shape `transition-core/v0` generates:
pure functions over field-less enums, with `match` and no loops, heap
collections, traits or interior mutability. The `composite/v0` scheduler
(`VecDeque`, `&mut self`) is outside the subset that was demonstrated. It is
connected to the composition proofs through a Lean model of it and bounded
conformance testing (kind 4 above). A proof would need an Aeneas-friendly
scheduler profile (fixed-capacity arrays), with Verus (kind 3) as a candidate
for its local properties.
