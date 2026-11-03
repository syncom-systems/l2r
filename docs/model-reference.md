# Architecture model reference (`l2r.architecture/v0`)

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

Unknown fields are rejected, so a misspelled field is an error rather than being
silently ignored. The order of components, messages and invariants does not
matter: the model is canonicalized before it is digested.

## Profiles

| Profile | Kind | Generated (L2R-owned) | Implementation (created once, never overwritten) |
| --- | --- | --- | --- |
| `crate-root/v0` | whole crate | `BUILD.bazel`, `src/lib.rs` | – |
| `transition-core/v0` | pure state machine | `mod.rs`: enums mirroring the Lean types, `init`/`step` contract; `contract_tests.rs`: totality over all states and inputs | `imp.rs` |
| `composite/v0` | container, owns no state | `mod.rs`: children, `Envelope`, `System` with one FIFO mailbox per machine child and a deterministic round-robin scheduler | `routes.rs`: one translation function per message edge |

Hierarchical composition comes from nesting composites. In v0, state types must be
Lean inductives whose constructors have no fields. Statecharts with nested states
inside one machine, and parameterized states, need a future profile.

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
| L2R-O001 | A Lean state type owned by more than one component |
| L2R-L001 | Unsupported inventory schema |
| L2R-L002 | Malformed Lean name |
| L2R-L003 | Lean name not in the inventory |
| L2R-L004 | Wrong declaration kind (e.g. a definition where a theorem is required) |
| L2R-L005 | State, input or output type is not a field-less inductive |
| L2R-L006 | Exact type mismatch for `init`, `step` or a translation |
| L2R-L007 | Constructor names that cannot become unique Rust variants |
| L2R-M001 | Duplicate message id |
| L2R-M002 | Message endpoint does not exist |
| L2R-M003 | Message endpoint is not a machine |
| L2R-M004 | Self-message |
| L2R-M005 | Endpoints are not siblings (strictest hierarchy rule; see the pending decision in ADR-0003) |
| L2R-M006 | Two edges between the same ordered pair |

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
  reviewed. It is **not** proof of identity or authorization.
- `l2r generate` refuses unless the recorded approval matches the current plan,
  and says what changed: the model, the Lean inventory, the generator, or the
  profiles.
- `l2r recover` finishes an interrupted generation of the approved plan.

The precise guarantees, assumptions and limits of writing are in
[`generation-boundary.md`](generation-boundary.md).
