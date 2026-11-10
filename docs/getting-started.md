# Getting started: a new project with L2R

This is the whole workflow for a system of your own, in a repository of your
own. It uses nothing from L2R's examples. `tools/e2e/new_project.sh` runs
exactly these steps, with every refusal, in CI.

## What you need

- [Bazel](https://bazel.build) through `bazelisk` (the project pins its version).
- Lean 4, to write the specification, export its inventory and replay
  conformance traces. The generator itself never runs Lean. Either install it
  ([`elan`](https://lean-lang.org/install), which provides `lake` and follows
  each project's `lean-toolchain` file), or use the release L2R pins, fetched
  by Bazel and verified by its sha256:

  ```sh
  export PATH="$(bazel run @l2r//tools/lean:bin_path)":$PATH
  ```

  That is the Lean every Lean file in L2R is checked with, the one `l2r init`
  writes into the new project's `lean-toolchain` files, and the one CI uses for
  the steps below.

## Who owns what

| Fact | Lives in | Owner |
| --- | --- | --- |
| Behaviour: states, inputs, outputs, transitions, invariants, what a message means | a Lean 4 project (`spec/`) | you, in Lean |
| Structure: components, containment, state ownership, ports, edges, capacities, Rust mapping | `architecture.toml` | you, the architect |
| What Lean elaborated | `lean-inventory.json` | exported, never written by hand |
| What was reviewed | `architecture.approval.json` | whoever records approvals in your repository |
| Contracts: types, signatures, mailboxes, the scheduler | `scaffold/` generated files | L2R; never edited |
| Implementation | `scaffold/**/imp.rs`, `routes.rs` | the implementer, human or agent |

## 1. Create the project

L2R is used as a Bazel dependency. For a new repository:

```sh
git clone https://github.com/syncom-systems/l2r && cd l2r
bazel run //crates/l2r_cli:l2r -- init ~/work/pump_station --id pump_station \
    --workspace-commit "$(git rev-parse HEAD)"
cd ~/work/pump_station && git init
```

`init` writes a complete, valid project and refuses to touch a directory that
already has any of its files:

```
MODULE.bazel  .bazelversion  .bazelrc  .gitignore   a Bazel workspace pinning L2R to that commit
architecture.toml                                    the model: one composite, one machine, three ports
spec/                                                the Lean project: PumpStation/Monitor.lean, Wiring.lean
lean-inventory.json                                  what that Lean exports
BUILD.bazel                                          the drift test, and the conformance recorder once derived
verification/                                        a Lake package for the derived network, conformance and your proofs
README.md                                            the loop, with this project's names
```

Without `--workspace-commit` (or `--workspace-path <l2r checkout>`) only the
project files are written, for a package inside an existing workspace that
already depends on `l2r`.

From here on:

```sh
L2R="bazel run @l2r//crates/l2r_cli:l2r --"
M=$PWD/architecture.toml          # bazel run changes directory: give absolute paths
```

## 2. Specify behaviour in Lean

A machine is five declarations in a namespace of your choice:

```lean
structure State where ...                      -- or an inductive
inductive Input where ...
inductive Output where ...
def init : State := ...
def step : State → Input → State × Option Output := ...
```

plus the theorems you want to hold. A message edge is a function
`Source → Option Target`; an input port a total function `Message → Target`.
The types may carry data from a closed set (`Bool`, `UInt8/16/32/64`, your own
non-recursive structures and inductives, `Option`); see
[`composite-v2.md`](composite-v2.md). Anything else is rejected by name.

Two things to decide deliberately:

- **Arithmetic.** `UInt8` addition wraps in Lean. If the count should stop at
  255, say so in Lean (`if n = 255 then 255 else n + 1`); the implementation
  must then do the same, and a refinement proof will hold it to that.
- **What is one machine.** A machine owns its state exclusively and changes it
  only in `step`. Anything two parties must both change is two machines and a
  message.

After every change to the Lean project:

```sh
$L2R inventory $PWD/spec --out $PWD/lean-inventory.json PumpStation
```

This builds the Lake project and exports the declarations Lean elaborated. The
inventory's digest is part of every plan, so a specification change always
shows up as a stale approval.

## 3. Describe the structure

Edit `architecture.toml`. The reference is
[`model-reference.md`](model-reference.md); hierarchy and ports are in
[`composite-v2.md`](composite-v2.md).

```sh
$L2R validate $M      # every finding at once, each with a stable rule code
$L2R render $M        # Mermaid: containment, ports on boundaries, edges
```

Validation checks structure and also every Lean reference: each name must exist
in the inventory with the right kind and the exact type. `step` must be
`State → Input → State × Option Output` for the component's own types; an edge's
translation must map its source's output type to its target's input type.

## 4. Review, approve, generate

```sh
$L2R preview $M                                   # every file, its owner, its exact content or diff
$L2R approve $M --digest <digest> --by "<name>"   # records that this exact plan was reviewed
$L2R generate $M                                  # writes it; refuses anything else
```

- The digest covers the model, the inventory, the generator and every planned
  byte. `approve` accepts only the digest of the current plan.
- `generate` refuses when anything changed since the approval and says what. It
  creates `imp.rs` and `routes.rs` once and never touches them again.
- Replacing or deleting an existing generated file needs that file named in the
  approval record, with the digest of its current content, which `preview` shows.
- An approval record says a plan was recorded as reviewed. It does not say by
  whom; that is your repository's review process
  ([`governance.md`](governance.md)).

Commit the model, the inventory, the approval record and the scaffold together.

## 5. Implement

One packet per component:

```sh
$L2R packet $M monitor
```

It lists the files that may be edited, the contract files to read, the Lean
declarations with their types, the theorems, the approved connections and the
rules. It is written to be handed to a coding agent as it is: the agent
implements inside the contract and reports contradictions instead of changing
the model, the specification or the tests.

```sh
bazel test //...
```

runs two kinds of test:

- the scaffold's tests: the generated contract tests (every transition is
  defined on representative values, including the boundary values of every
  integer field, so an overflow panics here) and whatever tests the implementer
  adds;
- `scaffold_matches_approved_plan_test` (`l2r_check_test`): no generated file
  was edited, the approval is current, no generation is pending, every
  implementation file is within its budget.

## 6. Change things

| You change | What happens |
| --- | --- |
| a Lean declaration | re-export the inventory; the approval is stale; preview shows what the change does to the contracts |
| the model | the approval is stale; preview, approve, generate |
| an implementation file | nothing to approve; tests and the budget apply |
| a generated file | `l2r check` reports drift; move the change into the model or Lean |
| a component is removed or moved | preview lists the generated files to delete and the implementation files left in place as orphans; nothing is deleted that the approval does not name |

If `generate` is interrupted, `l2r recover` finishes the approved plan.

## 7. Evidence that the code does what the specification says

Passing tests is not that evidence. L2R keeps the kinds apart
([`verification.md`](verification.md)); for your own project, in increasing
strength:

1. **Theorems about the specification** (Lean). You write them; the model names
   them as `invariants`; the packet shows them to the implementer.
2. **System properties over every interleaving.** `l2r lean-composition $M`
   prints the network your model approves, as Lean, wired to the generic
   semantics (`lean/L2R/Async1.lean`). State invariants about it with
   `L2R.Async1.reachable_invariant`; they then hold for the generated scheduler
   by `L2R.Async1.Ports.run_invariant`.
   `examples/tank_station/composition/TankStationProperties.lean` is the
   pattern for a system with data: sixty lines that lift one machine's theorem
   to every reachable configuration of the composed system, with no state
   enumeration. `examples/crossing_v1/composition` is a larger worked case for
   finite machines (safety, deadlock-freedom, liveness under stated fairness).
3. **The generated scheduler and your implementation behave like the Lean
   model, on what was tried.** `verification/README.md` in your project has the
   five commands: `l2r conformance` derives a recorder that drives the generated
   `System` (with your `imp.rs` and `routes.rs`) and a Lean program that replays
   every recorded sequence on the scheduler model over your Lean `step`
   functions. It needs no proof effort and catches a wrong transition, a wrong
   translation, a wrong route order or an overflow on the sequences and sample
   messages it tries. It is a test.
4. **The Rust computes what Lean says.** Translate the implementation back to
   Lean with Charon and Aeneas and prove refinement. `verification/aeneas` and
   `verification/aeneas_v1` are worked cases, with the checks that keep such a
   proof honest (source identity, pinned statements, axiom audit, kernel replay,
   seeded defects that must be caught).

What L2R gives every project without further work: the model is validated
against compiled Lean, the plan is deterministic and bound to its inputs, the
write boundary is safe, drift is detected, and the semantics and the scheduler
model your system is an instance of are proved once, generically. What it does
not give you automatically is steps 2 and 4 for *your* machines: those are
proofs you write, and until you do, the honest status of your implementation is
"tested" (step 3).

## Using L2R in CI

```yaml
- run: bazel test //...
```

is enough for the drift test and the contract tests. Nothing in the default
build fetches a verification toolchain.
