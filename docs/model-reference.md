# Architecture model reference (`l2r.architecture/v0`, `v1` and `v2`)

The model is the authoritative source for **software structure**. Behavior
(states, inputs, transitions, invariants, and what messages mean) is owned by
Lean, and the model refers to it only by qualified name. Every such name is
resolved against a **Lean declaration inventory** exported from compiled Lean
(`tools/lean_inventory`), so a reference to a declaration that Lean did not
elaborate is rejected.

See [`examples/crossing/architecture.toml`](../examples/crossing/architecture.toml)
for a complete model.

## Schema

```toml
schema = "l2r.architecture/v0"

[system]
id = "crossing"                      # lower_snake_case
name = "Pedestrian crossing"
rust_crate = "crossing_scaffold"     # generated crate and Bazel target name
lean_inventory = "lean-inventory.json"   # relative to this file; no `..`
output_dir = "scaffold"              # relative to this file; no `..`

[[component]]
id = "signal"                        # stable identity, unique
name = "Crossing signal"
parent = "crossing"                  # omitted only for the single root
profile = "transition-core/v0"       # fixes the component's kind and files
responsibility = "..."               # required, in the architect's words
rust_module = "signal"               # unique among siblings
notes = "..."                        # optional instructions to the implementer
implementation_budget_lines = 60     # optional; enforced by `l2r check`

[component.behavior]                 # machines only
owned_state = "Crossing.Signal.State"    # this component is its single owner
input = "Crossing.Signal.Input"
output = "Crossing.Signal.Output"
init = "Crossing.Signal.init"
step = "Crossing.Signal.step"            # State → Input → State × Option Output
invariants = ["Crossing.Signal.no_conflicting_release"]

[[message]]
id = "latch_to_signal"               # also the routing function's name
from = "request_latch"
to = "signal"
translation = "Crossing.Routes.latchToSignal"   # FromOutput → Option ToInput
```

### Schema `l2r.architecture/v1`

`v1` is `v0` plus what `composite/v1` needs; every `v0` model is also valid with
`schema = "l2r.architecture/v1"` and means the same. A document that declares
`v0` and uses any of the following is rejected (L2R-S015), so `v0` can never
silently acquire new meaning.

```toml
schema = "l2r.architecture/v1"

[[component]]
id = "crossing"
profile = "composite/v1"             # must be the root; children must be machines
# ...
[[component]]
id = "signal"
parent = "crossing"
profile = "transition-core/v0"
mailbox_capacity = 2                 # required for every child of a composite/v1; 1..=64

[[port]]                             # the only way the environment reaches the composite
id = "timer"                         # names the generated `send_timer`
composite = "crossing"
to = "signal"                        # a machine child
message = "CrossingV1.Ports.TimerEvent"        # field-less Lean inductive
translation = "CrossingV1.Ports.timerToSignal"  # TimerEvent → Signal.Input (total)
```

Capacity is part of the behavioral semantics (`l2r.async/v1`), not an
implementation detail: a full mailbox blocks the sender and rejects a port send;
nothing is dropped. See [`composite-v1.md`](composite-v1.md).

### Schema `l2r.architecture/v2`

`v2` is for hierarchical systems: composites nest, ports have a direction
(`in` or `out`), the root's `out` ports are bounded outboxes, and Lean types may
carry data. It uses its own two component profiles (`transition-core/v1`,
`composite/v2`) and needs an inventory with constructor fields
(`l2r.lean-inventory/v1`). The schema, the meaning of hierarchy and the
generated code are described in [`composite-v2.md`](composite-v2.md).

Unknown fields are rejected, so a misspelled field is an error rather than being
silently ignored. The order of components, messages and invariants does not
matter: the model is canonicalized before it is digested.

## Profiles

| Profile | Kind | Generated (L2R-owned) | Implementation (created once, never overwritten) |
| --- | --- | --- | --- |
| `crate-root/v0` | whole crate | `BUILD.bazel`, `src/lib.rs` | – |
| `transition-core/v0` | pure state machine | `mod.rs`: enums mirroring the Lean types, `init`/`step` contract; `contract_tests.rs`: totality over all states and inputs | `imp.rs` |
| `composite/v0` | container, owns no state | `mod.rs`: children, `Envelope`, `System` with one FIFO mailbox per machine child and a deterministic round-robin scheduler | `routes.rs`: one translation function per message edge |
| `crate-root/v1` | whole crate (schema `v2`) | `BUILD.bazel`, `src/lib.rs`, `src/types.rs`: one Rust type per Lean data type | – |
| `transition-core/v1` | pure state machine whose types may carry data (schema `v2`) | `mod.rs`: `State`/`Input`/`Output` as aliases into `types`, `init`/`step` contract; `contract_tests.rs`: definedness on representative values, including the boundary values of every field | `imp.rs` |
| `composite/v2` | container, root or nested, with typed `in` and `out` ports (schema `v2`) | `mod.rs`: children and boundary; for the root also bounded mailboxes and outboxes and the `System` scheduling the flat network of every machine (`send_<port>`, `take_<port>`, `step_once`) | `routes.rs`: one translation per edge living in the composite and per `in` port |
| `composite/v1` | root container, owns no state, machine children only | `mod.rs`: capacities, typed port message enums, array-backed bounded mailboxes, `System` with `send_<port>` (explicit `Rejected`), a guarded round-robin `step_once` reporting `Delivered`/`Settled`/`Blocked` | `routes.rs`: one translation per edge and per port |

In schemas `v0` and `v1`, state types must be Lean inductives whose constructors
have no fields, and only `composite/v0` nests. Schema `v2` lifts both limits:
composites nest with typed boundaries, and types carry data. Statecharts with
nested states inside one machine remain the specification's business: a machine
is whatever its Lean `step` says.

## Validation rules

Each finding carries a stable code. Validation reports every finding in one run
and never repairs the model.

| Code | Rule |
| --- | --- |
| L2R-S001 | Unsupported model schema |
| L2R-S002 | Invalid identifier (system, crate, component, module or message id) |
| L2R-S003 | Duplicate component id |
| L2R-S004 | Parent does not exist |
| L2R-S005 | Containment cycle |
| L2R-S006 | Not exactly one root component |
| L2R-S007 | Unsupported or unknown profile version |
| L2R-S008 | Profile shape: a machine needs behavior and no children; a composite needs children and no behavior |
| L2R-S009 | Two siblings map to the same Rust module |
| L2R-S010 | Reserved or keyword module name |
| L2R-S011 | Empty name or responsibility |
| L2R-S012 | Control characters in text |
| L2R-S013 | Unsafe path (absolute, `..`, hidden segments) |
| L2R-S014 | Zero implementation budget |
| L2R-S015 | Ports, capacities or `composite/v1` used by a `v0` document; a port `direction` or `capacity` in a `v0`/`v1` document |
| L2R-S016 | `composite/v1` shape: must be the root, children must be machines |
| L2R-S017 | A child of a `composite/v1` lacks `mailbox_capacity`, or it is 0 or above 64 |
| L2R-S018 | `mailbox_capacity` outside the children of a `composite/v1` |
| L2R-S019 | A `v2` profile in a `v0`/`v1` model, or an older profile in a `v2` model |
| L2R-S020 | Two identifiers that generate the same Rust name (`a_1` and `a1`; edge `port_x` and port `x`; port `q_state` and machine `send_q`) |
| L2R-S021 | A `composite/v0` or `composite/v1` whose generated code would not compile because two names meet: a child module named like a `System` field or method the profile generates (`next`, `run`, …), a module named like another child's derived member (`a_inbox`, `route_from_a`), two ids giving one `Envelope` variant (`a_1` and `a1`), an edge named like a port function (`port_x`), or a `composite/v0` child module named `std`. Added 2026-11-13 as a correction of the frozen schemas: it rejects only models whose output could not compile (ADR-0005 D9 clarification) |
| L2R-O001 | A Lean state type owned by more than one component |
| L2R-L001 | Unsupported inventory schema; a `v2` model with an inventory that lacks constructor fields |
| L2R-L002 | Malformed Lean name |
| L2R-L003 | Lean name not in the inventory |
| L2R-L004 | Wrong declaration kind (e.g. a definition where a theorem is required) |
| L2R-L005 | State, input or output type is not a field-less inductive; in schema `v2`, a type outside the supported data subset |
| L2R-L006 | Exact type mismatch for `init`, `step` or a translation |
| L2R-L007 | Constructor, field or type names that cannot become unique Rust names (collisions, keywords, a type called `Option`, `Vec`, `Box`, `String` or `Result`) |
| L2R-L008 | A recursive data type (schema `v2`) |
| L2R-M001 | Duplicate message id |
| L2R-M002 | Message endpoint does not exist |
| L2R-M003 | Message endpoint is not a machine |
| L2R-M004 | Self-message |
| L2R-M005 | Endpoints are not siblings (ADR-0003 D7). In schema `v2`: the endpoints are not both visible in one composite; boundaries are crossed through ports |
| L2R-M006 | Two edges between the same ordered pair |
| L2R-M007 | A machine in a `composite/v1` has no outgoing edge, so its outputs would be silently discarded |
| L2R-M008 | An output that can return to its own machine's mailbox through edges and ports (schema `v2`) |
| L2R-M009 | One output reaching more than 32 receivers (schema `v2`) |
| L2R-M010 | An edge that can never carry a message because nothing feeds its source (schema `v2`) |
| L2R-P001 | Duplicate port id |
| L2R-P002 | A port's composite does not exist or is not a `composite/v1` |
| L2R-P003 | A port's target is not a machine inside its composite |
| L2R-P004 | A `composite/v1` has no port; a `v2` root has no `in` port |
| L2R-P005 | A `composite/v1` port whose Lean message type cannot get its own Rust type: distinct Lean types with one last segment (`A.Command` and `B.Command`), a segment that is not a usable Rust type name (`Timer'`, `Self`), one named like a type the module generates (`System`, `<Child>Inbox`, …), or one naming a standard type the module uses (`Option`, `Result`, `Default`). Added 2026-11-13 as a correction of the frozen schemas (ADR-0005 D9 clarification) |
| L2R-P006 | A port and a component share an identifier (schema `v2`) |
| L2R-P007 | A `v2` port without `direction = "in"` or `"out"` |
| L2R-P008 | `capacity` missing on a root `out` port, out of range, or used on any other port |
| L2R-P009 | An `out` port with `to` or `translation` |
| L2R-P010 | A nested `in` port that nothing feeds (schema `v2`) |

## Recurring failure modes, turned into rules

| Failure mode seen in agent-built systems | Rule in L2R |
| --- | --- |
| Ambiguous state ownership | Every machine names one `owned_state`; a second owner is L2R-O001. The composite exposes child state read-only, and only the child's `step` changes it. |
| Duplicated state | Composites own no state (L2R-S008). State types come from Lean, never restated in the model. |
| Architectural drift | The plan digest binds model, inventory, generator and profiles. Generated files that differ from the plan are reported by `l2r check` and refused by `l2r generate`. |
| Oversized agent-written modules | One implementation file per component, plus an optional line budget that `l2r check` enforces. |
| Agents silently changing structure | Generated contracts are L2R-owned. The implementation must satisfy their signatures to compile, and editing them is detected. |
| Unverified refinement claims | Generated docs state that Rust tests do not establish correspondence. Proof status is reported per obligation (see `verification/aeneas`). |
| Formal model and deployment diverge | The inventory comes from compiled Lean and its digest is bound into the plan, so a changed inventory makes the approval stale. Whether the committed inventory matches the Lean sources is checked in CI by the proof lane (`bazel test --config=verify //verification/...`). |

## Approval and write semantics

- `l2r plan` and `l2r preview` are read-only. The preview shows each file's action
  (create, update, unchanged, keep, delete, orphan), any conflicts, and the exact
  content or diff of every change.
- `l2r approve --digest D --by NAME` records an approval **only if** `D` is the
  current plan digest. A record is evidence that this exact plan was recorded as
  reviewed. It is **not** proof of identity or authorization. It also lists
  (`authorized_changes`) each existing generated file the plan would replace or
  delete, with the digest of its current content, taken from the preview.
  `generate` replaces or deletes nothing the record does not name; the lock in
  the output tree is evidence only.
- `l2r generate` refuses unless the recorded approval matches the current plan,
  and says what changed: the model, the Lean inventory, the generator, or the
  profiles.
- `l2r recover` finishes an interrupted generation of the approved plan.

The precise guarantees, assumptions and limits of writing are in
[`generation-boundary.md`](generation-boundary.md).

## Project layout and `l2r check`

The `l2r` command is the one supported front end. It, not a library, decides
where a project's files are and what `l2r check` means. The libraries
(`l2r_validate`, `l2r_plan`, `l2r_apply`) are not a public API.

**Layout.** Every path is relative to the directory of the model file:

| File | Where | Decided by |
| --- | --- | --- |
| Model | the path given to `l2r` (e.g. `architecture.toml`) | the caller |
| Lean inventory | `system.lean_inventory` | the model (rule S013: relative, no `..` or hidden segments) |
| Output tree | `system.output_dir` | the model (rule S013) |
| Approval record | `<model stem>.approval.json` beside the model (`architecture.toml` → `architecture.approval.json`) | the CLI (`crates/l2r_cli/src/session.rs`) |
| Lock and journal | inside the output tree | `l2r_apply` |

The CLI checks rule S013 before it opens the inventory, and validation checks
it again with every other rule. Both use one definition
(`l2r_validate::validate_system_paths`). The approval record's name is also
written in `bazel/l2r.bzl` and in `l2r_apply`'s crash-test binary. Neither can
use the CLI's code (one is Starlark, the other sits below the CLI). A change of
the name must change all three.

**`l2r check <model>`** fails, listing every finding, when any of these holds:

1. the model or the inventory does not load, the model is invalid, or it cannot be planned;
2. the output tree does not exist;
3. a planned file (generated or implementation) is missing, a generated file is
   out of date with the plan or no longer planned, or an implementation file is
   no longer planned;
4. the tree has a conflict generation would refuse: a symbolic link, a foreign
   or edited file where a generated one belongs, an invalid lock, a pending
   journal of an interrupted run, … (the full list is G4 in
   [`generation-boundary.md`](generation-boundary.md));
5. there is no lock, or it records another plan;
6. an implementation file exceeds its component's
   `implementation_budget_lines`;
7. there is no approval record, it cannot be read or has an unsupported format,
   or it is not for the current plan digest (the model, the inventory, the
   generator, a profile or the planned files changed).

Items 2 to 6 are `l2r_apply::check`. Item 7 is added by the CLI. A program that
calls `l2r_apply::check` directly gets no approval check. `l2r_check_test`
(`bazel/l2r.bzl`) runs `l2r check` on a copy of the project that Bazel makes
with symbolic links resolved, so a link in the output tree is not seen there;
run `l2r check` in the project itself for that.
