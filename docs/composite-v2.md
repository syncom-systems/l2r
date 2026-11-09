# Hierarchical composition: `composite/v2`, `transition-core/v1`, model schema `v2`

Model schema `l2r.architecture/v2` is for systems that are more than two
machines in one box. It adds three things, all as **new versions beside the
unchanged ones** (`v0` and `v1` models plan to exactly the bytes they did):

1. **Hierarchy.** Composites nest to any depth. A composite has a typed
   boundary: nothing reaches its children, and nothing leaves them, except
   through a port the architect declared.
2. **Output ports.** A system can tell its environment something: each `out`
   port of the root is a bounded outbox that the environment empties.
3. **Data.** States, inputs, outputs and port messages may be Lean structures
   and inductive types with fields, over a closed set of field types.

Worked examples: [`examples/tank_station`](../examples/tank_station) (a nested
subsystem, forwarded ports, two outboxes, data in every message) and
[`examples/relay`](../examples/relay) (a generator fixture: three levels, port
chains, one output reaching one receiver twice, wrapping arithmetic).

## The decisions

| Concern | Decision | Where it is stated or checked |
| --- | --- | --- |
| What hierarchy means | **Hierarchy is structure; behaviour is the flat network of the machines.** A nested composite owns no state and runs no scheduler. The semantics of a `v2` model is the `l2r.async/v1` network whose children are all its machines, in identifier order. No new semantics was written for hierarchy. | `l2r_validate::flat`; the derived network (`l2r lean-composition`) |
| Crossing a boundary | Only through ports. An edge lives in one composite and joins things visible there: its child machines, the ports of its child composites, and its own `out` ports. This is the "siblings only" rule of ADR-0003 D7, kept, with ports as the sanctioned way across. | rules M002–M005, P003 |
| Input ports | An `in` port forwards to exactly one target (a child machine or an `in` port of a child composite) through a **total** Lean translation. A chain of them is composed. Fan-out is expressed by a machine, not by a port. | rule P003; `inject` in the derived network |
| Routes | Each output of a machine is carried along every path of approved edges and ports to a machine's mailbox or to an outbox of the root. A route is the composition of its translations; an edge translation returning `none` ends it (the Lean specification's own, visible decision). Routes are computed once and used, in the same order, by the generated scheduler and the derived Lean network. | `Flat::leaves[..].routes`; `route` in the derived network |
| Output ports | An `out` port of the root is a **child of the network**: its mailbox is the outbox, it has no state and emits nothing. A sender is blocked while the outbox is full (backpressure, nothing dropped). **The environment taking a message is a delivery step of that child**, which the semantics permit at any time. Every theorem of `l2r.async/v1` therefore applies unchanged, and a property proved over all interleavings holds however, and whether, the environment empties the outboxes. | `L2R.Async1.outboxStep`, `lean/L2R/Async1Ports.lean` |
| The scheduler | Round-robin over the **machines only**. Outboxes are emptied by the caller (`take_<port>`), never by `step_once`. `Settled` means no mailbox holds a message; `Blocked` means some does and no machine's delivery is enabled. | `L2R.Async1.Ports`: `run_sound`, `stepOnce_delivered_iff`, `stepOnce_settled_iff`, `stepOnce_blocked` |
| No silent discard | A machine without an outgoing edge, and an `out` port of a nested composite that no edge of the parent reads, are rejected. | rule M007 |
| Nothing dead | An edge or a nested `in` port that no message can ever pass (nothing feeds its source) is rejected: it would be approved and documented but generate nothing. | rules M010, P010 |
| No feedback through the hierarchy | An output that could return to its own machine's mailbox is rejected; such a loop belongs inside the transition function. | rule M008 |
| Data types | A closed subset, rejected rather than approximated outside it: a non-recursive inductive type or structure in `Type`, without parameters or indices, whose fields are `Bool`, `UInt8/16/32/64`, another such type, or an `Option` of one. Each Lean type has **exactly one** Rust type, in `src/types.rs`, at the path its Lean namespace gives. A type may not be called `Box`, `Option`, `Result`, `String` or `Vec`, and no namespace segment or field may be a Rust keyword. | `l2r_validate::data`; rules L005, L007, L008 |
| Generated names | Every name the generator derives from an identifier is checked for collisions before anything is planned: type prefixes, the members of `System` (`send_<port>`, `take_<port>`, `<machine>_state`, …) and the functions of each `routes.rs`. | rules S010, S020 |
| Integer arithmetic | Lean's fixed-width integers wrap; Rust's `+` panics in a build with overflow checks and wraps silently in one without. The specification decides (write a saturating `bump` in Lean if that is what is meant) and the implementation must match. The generated contract tests run every transition on sample states and inputs in which every field takes each of its boundary values (0, 1 and the largest, for an integer) at least once, the other fields held at their first value, and once all at their last. So a `+` that overflows at a field's largest value fails a test in a checked build (Bazel's default `fastbuild`, and `dbg`; not `opt`). Combinations of boundary values are not covered. That is a test, not a proof. | `contract_tests.rs`; `examples/relay` (wrapping), `examples/tank_station` (saturating) |

## The model

```toml
schema = "l2r.architecture/v2"

[[component]]
id = "station"
profile = "composite/v2"            # the root: generates the scheduler
# ...
[[component]]
id = "sensing"
parent = "station"
profile = "composite/v2"            # nested: a boundary, no scheduler
# ...
[[component]]
id = "filter"
parent = "sensing"
profile = "transition-core/v1"
mailbox_capacity = 2                # every machine states one; 1..=64
[component.behavior]                # as before; the types may carry data
# ...

[[port]]                            # the system's input
id = "sample"
composite = "station"
direction = "in"
message = "Tank.Ports.Sample"
to = "reading"                      # an `in` port of the child composite `sensing`
translation = "Tank.Ports.sampleToSensing"     # Sample → Sample (total)

[[port]]                            # a nested composite's output
id = "level_changed"
composite = "sensing"
direction = "out"
message = "Tank.Level"

[[port]]                            # the system's output: a bounded outbox
id = "valve_command"
composite = "station"
direction = "out"
message = "Tank.Ports.ValveCommand"
capacity = 2

[[message]]                         # inside `sensing`: machine → own out port
id = "filter_to_level"
from = "filter"
to = "level_changed"
translation = "Tank.Routes.filterToLevel"      # Filter.Output → Option Level

[[message]]                         # inside `station`: child's out port → machine
id = "level_to_controller"
from = "level_changed"
to = "controller"
translation = "Tank.Routes.levelToController"  # Level → Option Controller.Input
```

Edge endpoints are identifiers of machines or ports, so a port and a component
may not share one. `l2r render` draws the ports on their composites' boundaries.

The inventory must be `l2r.lean-inventory/v1` (constructor fields):

```sh
tools/lean_inventory/export.sh --v1 <lake-project-dir> <output.json> <RootModule>...
```

## The generated code

| File | Owner | Contents |
| --- | --- | --- |
| `src/types.rs` | L2R | One `Copy` type per Lean data type, in modules mirroring the Lean namespaces. A structure becomes a `struct` with named fields, an inductive an `enum` with positional fields. Test-only `samples()` give representative values. |
| `<machine>/mod.rs` | L2R | `State`, `Input`, `Output` as aliases into `types`, and the `init`/`step` contract. |
| `<machine>/contract_tests.rs` | L2R | `step` is defined (does not panic; with overflow checks, does not overflow) on representative states and inputs. |
| `<machine>/imp.rs` | implementer | The transition. |
| `<composite>/mod.rs` | L2R | The children and the documented boundary. For the root: the flat network, bounded mailboxes and outboxes, and `System`. |
| `<composite>/routes.rs` | implementer | One function per edge that lives in the composite and per `in` port it has. Present only if there is one. |

`System` has `send_<port>` (returns `Err(Rejected)` when the target's mailbox is
full), `take_<port>`, `step_once`, `run`, and read-only access to every
machine's state, mailbox and outbox. As in `composite/v1` it is straight-line
code over fixed-size arrays, shaped for Rust-to-Lean translation.

## What is established, and how

| Claim | Kind | Evidence |
| --- | --- | --- |
| Everything `l2r.async/v1` proves (no overflow of any mailbox **or outbox**, invariants over permitted behaviour, the fair-termination principle) holds for the flat network of a `v2` model | **Proved** (generic; the network is an ordinary `L2R.Async1.Network`) | `lean/L2R/Async1.lean` |
| Every run of the Lean model of the generated scheduler, with the caller sending what the environment may and taking from outboxes whenever it likes, is a permitted behaviour | **Proved** (generic) | `Ports.run_sound`, `take_sound` |
| That scheduler serves machines only, delivers whenever a machine's delivery is enabled, and reports `Settled` and `Blocked` exactly | **Proved** (generic) | `Ports.stepOnce_delivers`, `stepOnce_delivered_iff`, `stepOnce_settled_iff`, `stepOnce_blocked` |
| The derived network of each example is the one the approved model determines | **Checked**: re-derived and compared byte for byte; its header is tied to the approval record and to the semantics and scheduler sources; its routes, capacities and ports are pinned in the audit | `l2r_plan_v2_test`; `verification/composition_v2` gates 0 and 2 |
| The committed inventories are what the compiled specifications export | **Checked** in the proof lane | `verification/composition_v2` gate 4 |
| The machines' own invariants (the interlock, the filter's confirmation, the alarm's latch; the relay's and the starter's) | **Proved** about the Lean specifications; each is audited (standard axioms only, statement pinned) and kernel-replayed in the lane | `examples/*/spec`; `verification/composition_v2` gates 2 and 3 |
| The generated Rust `System` behaves as the Lean scheduler model | **Tested** (bounded). `l2r conformance` derives a recorder for the Rust and a replayer for the Lean model; for each example every operation sequence of length 4 and 3,000 pseudo-random sequences of length 80 agree on every result, every machine state, every mailbox and every outbox after every operation. A scheduler model with a different but still sound order is rejected. There is **no refinement proof** for `composite/v2` | `examples/*/conformance`; `verification/composition_v2` gate 7 |
| The Rust machines and routes of the `v2` examples compute what their Lean specifications say | **Tested, not proved**: the same conformance run drives the real `imp.rs` and `routes.rs` against the Lean `step` and translations, for the sample messages (integers 0, 1, the middle and the largest value) and the states those sequences reach. No Rust-to-Lean refinement proof exists for a `transition-core/v1` machine | as above; `contract_tests.rs` |
| Tank station: in every reachable configuration, a controller that knows the level is high holds the valve closed; for **every** environment and every interleaving, including any order of the environment taking from outboxes | **Proved** symbolically (no state enumeration: the machines carry data), by lifting the machine's own theorem with `reachable_invariant`; transferred to every run of the scheduler model by `Ports.run_invariant` | `examples/tank_station/composition/TankStationProperties.lean`: `interlock`, `scheduler_interlock` |
| Tank station: liveness, absence of blocking, any property relating several machines or an outbox's contents | **Not proved.** `Blocked` is reachable by design when the environment does not empty an outbox | — |
| Real sensors, actuators, timing | **External assumption** | — |

## Conformance, for every project

```sh
l2r conformance <model> --rust > verification/traces.rs        # drives the generated System
l2r conformance <model> --lean > verification/Conformance.lean # replays on L2R.Async1.Ports
```

Both are derived from the validated model, record the digests of what they were
derived from, and are not part of any plan. An operation is a send of one sample
message to an input port, a `step_once`, or a take from an output port. The
sample messages are enumerated by the generator from the port's Lean type, the
same values in both languages. Observations are compared as text: the Rust side
prints `Debug`; the Lean side has a generated renderer per data type that prints
the same. `l2r init` sets up the Lake package and the Bazel target that run it
(`verification/README.md` in a new project). What a passing run means is in the
table above: it is a test.

## Limits

- An `in` port has one target; a root `in` port therefore reaches one machine.
- Every machine and every root `out` port has a capacity of at most 64.
- One output reaches at most 32 receivers.
- No failure semantics (loss, reordering, crashes, time), as before.
- The data subset has no `Nat`, `Int`, strings, lists, arrays or recursive types.
- `v2` models cannot use `transition-core/v0`, `composite/v0` or `composite/v1`
  components, and the older schemas cannot use the `v2` profiles.
