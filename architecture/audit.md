# Architecture audit: L2R, all crates

Architect: the repository owner. Date: 2026-11-13. Knowledge base: `architecture-knowledge/` (rulings first).

Evidence labels: MEASURED (deps.py, compiler, build, a command that was run), READ (`file:line`), INFERRED (chain stated), ARCHITECT (answer, date).
Marks: PROPOSED · DECIDED (architect, date) · OPEN.

**Status (2026-11-13):** the architect answered Q1–Q7 on PR #21. The decisions are quoted verbatim under [Decided](#decided) and are authoritative. Remedies are delivered by separate engineering PRs; this record changes no code.

## Verdict

**The crate structure is appropriate. Its one structural weakness is that the concept along which L2R evolves, the versioned profile and schema, has no owning type.**

What holds:
- **Crate layering.** The crates form a loop-free uses relation with five levels: `l2r_model` < `l2r_validate` < `l2r_plan` < `l2r_apply` < `l2r` (the CLI). `l2r_testing` is test-only and uses `l2r_model` alone (MEASURED). Those are the five crates `README.md:121-127` lists, in the same order. The README states no dependencies and does not list `l2r_testing`.
- **The validation boundary.** `Validated` can be constructed only by `l2r_validate`, and `plan()` accepts nothing else (READ `l2r_validate/src/resolved.rs:16-23,55,71`; `l2r_plan/src/lib.rs:58`).
- **Authority in `l2r_apply`.** It does not rely on validation. Paths are `RelPath`, approvals are matched by the single `Approval::authorizes`, and deletion is checked against the generated-file marker on the actual bytes (findings F1–F7; ARCHITECT 2026-11-13 on F1).
- **Decomposition in `l2r_plan`.** The crate is split by profile version, which fits the binding rule that published profiles are immutable (AGENTS.md).

What does not hold:
- **No owner for what a profile is.** A profile is a `String` (READ `l2r_model/src/architecture.rs:74`). What a profile *is*, and which names its output uses, are rewritten in each module that needs them, across `l2r_validate`, `l2r_plan` and the CLI (F8, F9, F10).
- **The likely change multiplies the cost.** Under immutability every new behaviour is a new version. The repository added such versions on 2026-11-02, 11-03 and 11-10 (INFERRED from `git log` on `l2r_model/src/profiles.rs` and the ADRs). So the most likely change is the one these findings make expensive.
- **The cost has already been paid once.** Commit 05e855e records a gate that had been missed (READ).
- **Two reachable defects come from the same cause.** In each, a validated model plans Rust that is wrong (F9). One is reproduced; the other is inferred.

## Architecture as built

This is what each module hides, as read. The README's crate table is the only other description of the design. Weak seams point to their findings.

| Crate (level) | Module | Hides |
|---|---|---|
| `l2r_model` (0) | `architecture` | The TOML model and its canonical order. One type serves every schema (F10), and a profile is a `String` (F8). |
| | `lean` | The inventory types and schema identifiers. |
| | `load` | Parsing of TOML and JSON. |
| | `digest` | The `sha256:<hex>` text and canonical JSON. |
| | `path` | `RelPath`, the one abstract type with a private representation: the path-safety rule, plus the reserved state-file names (F2). |
| | `profiles` | The profile identifiers and `SUPPORTED`. It does not hide what a profile is (F8). |
| `l2r_validate` (1) | `lib` | The schema dispatch. |
| | `structure`, `messages`, `ports`, `lean_refs` | The v0/v1 rules. `lean_refs` also holds the Lean type-print contract (ADR-0003 D2). |
| | `v2/*` | The v2 rules. `generated_names` restates the planner's names (F9). |
| | `data` | The Lean data subset and its Rust mirrors. |
| | `flat` | The flat network of ADR-0005 D1, D4. |
| | `resolved` | `Validated`, the only input the planner accepts (F14). |
| | `names`, `diagnostic` | The identifier syntax, and the findings with their stable codes. |
| `l2r_plan` (2) | `profiles/*` | One module per frozen v0/v1 profile, each owning its output text. |
| | `v2/*` | One module per v2 profile. `system/{buffer,route,attempt}` hides the scheduler code's shape. |
| | `text`, `layout` | The generated-file marker (one owner, shared with `l2r_apply`). The output helpers and directory layout are shared across versions (F11). |
| | `lean_composition`, `lean_composition_v2` | The derived Lean networks, plus the identities of the semantics sources (F15). |
| | `conformance/*` | The trace protocol. `value` and `renderers` are single owners. |
| | `lib`, `plan` | The schema choice, made by proxy (F8). `Plan` is a record with public fields, whose invariants `lib` establishes. |
| `l2r_apply` (3) | `tree`, `transaction`, `journal`, `lock`, `assess`, `approval`, `check`, `fault` | Descriptor-relative tree access; roll-forward writing; the transition table; authorization by the approval record (F1–F7). |
| `l2r` (4) | `main`, `commands`, `session`, `report`, `packet`, `diff`, `init`, `inventory` | The command syntax and messages. It also decides the project layout (F13), the render's own profile classification (F8), the packet's copy of the route naming (F9), and its own file writers (F12). |
| `l2r_testing` (test-only) | `lib` | Where the committed example inputs live, and the scratch-directory policy. |

**Is this an appropriate design?**
- **The crate level is.** Each crate hides one stage, and every edge points down (MEASURED).
- **Inside the crates it is mostly by decision, not by processing step:**
  - per profile version in `l2r_plan`;
  - per rule family in `l2r_validate`;
  - per mechanism in `l2r_apply`.
- **The exception is the versioned profile and schema** (F8–F11). That is where the next change will land.

## Scope and evidence

- **In scope:** `crates/*/src`: `l2r_model`, `l2r_validate`, `l2r_plan`, `l2r_apply` (with its `l2r_crash_after` test binary), `l2r` (`crates/l2r_cli`) and `l2r_testing`.
- **Out of scope:**
  - generated and derived files (`examples/*/scaffold/**`, `conformance/**`, `composition/**`), which are judged through their generator;
  - Lean (`lean/`), which owns behaviour;
  - `tools/`, `verification/` and `bazel/`, which were read only where cited.
- **Binding decisions read:**
  - `AGENTS.md` and `README.md`.
  - ADRs, fetched through `gh`:
    - read in full: ADR-0003 D1–D6 and ADR-0005 D1–D9;
    - only their decision headings read: ADR-0001, ADR-0002 and ADR-0004.
  - `docs/generation-boundary.md`, `docs/composite-v2.md` and `docs/model-reference.md` where cited.
  - Not read: the PRD (Discussion #1).
- **The immutability rule** is stated in `AGENTS.md` and in ADR-0005 D9 ("`v0` and `v1` are unchanged (ADR-0003 D8)"). Where the cited text is (READ, Discussions fetched 2026-11-13):
  - ADR-0003's accepted body (Discussion #5, ratified 2026-11-09) has D1–D7 only.
  - "New D8 — Profile and schema evolution" ("Published profile and schema versions are immutable…") is in ADR-0003's **r4 ratification-proposal comment** (2026-11-04), not in the body.
  - The same rule is proposed as an append to ADR-0001 D5 in ADR-0001's r3 and r4 comments. ADR-0001's accepted body D5 states determinism only: "The same canonical input, pinned generator and profiles must generate identical output bytes".
  - Both ratification records say "Historical Proposed/Draft wording is superseded by this ratification" and do not say whether the r4 texts were included.
  - The citation is corrected by a new comment on Discussion #16 (the architect's instruction, "Other findings" under [Decided](#decided)); D9's own sentence is accepted text, so the decision's substance is unchanged.
- **Measurement:** `deps.py --repo . --scope crates --out architecture/audit` exits 0. Crates come from `bazel query`; Cargo was not consulted.
  - Size: 7 crates, 67 modules, 14 crate edges, 219 module edges.
  - Loops: no crate loops. Three module loops: approval→assess→journal (F4), fault↔tree (F6), and lean_composition↔lean_composition_v2 (F15).
  - Checks: no unresolved dependencies, no references to undeclared crates, no declared dependency without a reference.
- **Not measured:** test targets, test modules, macro bodies and expansions. Generated sources and `select()` branches are named in `not_measured` but do not occur in `crates/`. Evidence from tests is READ.
- **Not in the build:** every library target is `//visibility:public`, except the fault-injection twin and the crash binary of `l2r_apply` (MEASURED `crates/*/BUILD.bazel`). The layering above therefore holds by convention, not because the build enforces it (F16).
- **Method:** three reviewers read the crates independently: `l2r_model`/`l2r_validate`, `l2r_plan`, and the CLI with `l2r_testing`. Each candidate kept below was then re-read at the cited lines. Reproductions say so where they were run.
- **F1–F7:** carried over from the `l2r_apply` audit (PR #20, issuecomment-6850881232). `crates/` has not changed since (MEASURED `git log 691736b..main -- crates/` is empty), so their citations stand.

## Findings

Ordered by the cost of the change each makes expensive, multiplied by its likelihood. F1–F7 keep the numbers they were given in the `l2r_apply` audit.

| Rank | Id | One line | Reachable today? |
|---|---|---|---|
| 1 | F8 | What a profile is (kind, schema) is decided separately in each module | no |
| 2 | F9 | The generated-Rust naming scheme is written in the planner and again in three places | **yes**: one reproduced, one inferred |
| 3 | F10 | One `Model` type for every schema; per-version field gates are written by hand | no (missed once, 05e855e) |
| 4 | F11 | Frozen profiles share output helpers with versions still changing | no |
| 5 | F12 | Three mechanisms write files beside the model; one follows symlinks | **yes** (inferred) |
| 6 | F13 | The project layout and the meaning of `check` are decided in the CLI | no |
| 7 | F14 | Guarantees carried by `Validated` are dropped inside the planner | no |
| 8 | F1–F7 | `l2r_apply` (carried over) | see the PR #20 record |
| 9 | F15 | Module loop `lean_composition ↔ lean_composition_v2` | no runtime effect |
| 10 | F16 | The build does not enforce the crate layering | no |

### F8: What a profile is (kind, schema) is decided separately in each module that needs it

- **Principle:** `parnas-1972-information-hiding`, Interpretation 2 and the Application bullet on a rule written in several modules; `liskov-zilles-1974-abstract-data-types`, the Application bullet on raw strings whose meaning one module's representation gives.
- **Evidence:**
  - READ: `l2r_model::profiles` holds the identifier constants and `SUPPORTED` (`l2r_model/src/profiles.rs:8-39`). Its own doc says "A profile fixes both the kind of a component and the exact files generated for it" (`:3`), but it maps no profile to a kind or to a schema.
  - READ: kind and schema are decided at each use, by comparing strings:
    - `is_machine` is defined three times: `l2r_validate/src/v2.rs:94`, `l2r_validate/src/flat.rs:153` and `l2r_plan/src/profiles/composite.rs:60`.
    - `is_composite` is defined twice: `l2r_validate/src/v2.rs:98` and `l2r/src/report.rs:203-210`.
    - Inline comparisons: `l2r_validate/src/structure.rs:16-17, 187-256` (ends `_ => {}`), `messages.rs:64`, `ports.rs:43,60,76`, `v2/components.rs:27-28`; `l2r_plan/src/lean_composition.rs:75,111`.
  - READ: the planner does not read `Model.schema`. It infers the schema from `validated.flat().is_some()` (`l2r_plan/src/lib.rs:65`, `lean_composition.rs:68`, `conformance.rs:78`).
- **Single implementation searched for:** none. `SUPPORTED` answers membership only.
- **Change scenario:** schema v3, built on v2's hierarchy (so its validator also produces a `Flat`), with a new composite profile. The edits it forces:

  | Edit | Kind |
  |---|---|
  | `SUPPORTED`, `SUPPORTED_SCHEMAS`, the validate dispatch (`l2r_validate/src/lib.rs:30`) | TESTED (S001, S007) |
  | The planner's profile dispatch (`l2r_plan/src/v2/mod.rs:139-145`) | loud `PlanError` at plan time |
  | The schema choice in `l2r lean-composition` (`lean_composition.rs:68-70`: any `Flat` goes to the v2 deriver) and in conformance (`conformance.rs:78-80`) | SILENT at derivation. Neither deriver reads a profile or the schema (READ), so a v3 model is derived under the identifiers `l2r.lean-composition/v2` and `l2r.conformance/v1`. That touches the derived Lean network, the behaviour side. It is caught later only if an example uses v3 and the `--config=verify` lane runs (READ `verification/composition_v2/BUILD.bazel:6`) |
  | `l2r/src/report.rs:203-210, 223` | SILENT. A v3 composite is drawn as a machine and its subtree disappears from `l2r render` |
  | `l2r_validate/src/structure.rs:16-17` (v2 profiles barred from v0/v1 models) | Loud, but late. A v0/v1 model naming the new profile passes validation and then fails in the planner with "has no implementation", which `lib.rs:44-45` calls a generator defect |

- **Reachable today?** No. The current lists match the current profiles (READ `structure.rs:16-17`, `report.rs:205-207`).
- **Cost:** every new profile or schema version, the change this repository makes most often (MEASURED `git log` on `l2r_model/src/profiles.rs` and `architecture.rs`: 64c9a17 on 2026-11-02, a4ab4c5 on 11-03, 219dffa on 11-10). The worst silent edit gives a derived Lean network under the wrong format identifier.
- **View:** `audit_F8`
- **Mark:** DECIDED (architect, 2026-11-13): Q1 YES.

### F9: The generated-Rust naming scheme is written in the planner and again in validate, the CLI and conformance

- **Principle:** `parnas-1972-information-hiding`, the Application bullet on a naming scheme written in several modules; `parnas-1979-uses-hierarchy`, the Application bullet on direction. `l2r_validate`, a lower module, encodes the vocabulary of the output of `l2r_plan`, a higher one.
- **Evidence:**
  - READ: the planner writes the names inline:
    - `System` members in `l2r_plan/src/v2/system.rs:252-404` and `v2/system/attempt.rs:18,54`;
    - route functions in `v2/mod.rs:114-119` (`hop_function_name`, `pub(crate)`);
    - type prefixes in `text.rs:47-59`.
  - READ: restatements elsewhere:
    - `l2r_validate/src/v2/generated_names.rs:59-77, 118-123` (rule S020's member lists), with a second case conversion, `names::upper_camel` (`names.rs:49-66`);
    - `l2r/src/packet.rs:70-75`, which copies `hop_function_name` line for line because that function is crate-private;
    - `packet.rs:238-252`, which mirrors `v2/mod.rs:98-111`;
    - `l2r_plan/src/conformance/recorder.rs:80-121`, which restates the accessor, send and take names.
  - READ: the copies have already drifted once. Commit 05e855e: "S020: collisions … are computed from the full set of names, not from three reserved identifiers."
  - Reachable defect 1:
    - MEASURED, with `l2r` built from this commit (`bazel build //crates/l2r_cli:l2r`): copy `examples/crossing_v1` to a scratch directory, and in its `architecture.toml` change `rust_module = "signal"` to `rust_module = "next"`. `l2r validate architecture.toml` reports "model `crossing_v1` is valid" (exit 0). `l2r preview architecture.toml` then plans a `System` struct with two fields named `next` (`next: next::State` and `next: usize`).
    - INFERRED: Rust rejects a struct with two fields of the same name (E0124). The generated crate was not compiled.
    - READ: v0/v1 validation reserves only `contract_tests`, `imp` and `routes` (`l2r_validate/src/names.rs:4`). S020 exists only for v2.
  - Reachable defect 2 (INFERRED, not run): `composite/v1` names a port's message type by the last segment of its Lean name (READ `l2r_plan/src/profiles/composite_v1.rs:111-117`). It emits one enum per Rust name and skips repeats (`:212-220`). Two ports carrying `A.Command` and `B.Command` would share one `Command` enum mirroring `A.Command`. The derived Lean uses full names (`lean_composition.rs:283-285`), so the Rust and the Lean would disagree. READ: no v0/v1 rule rejects this (`l2r_validate/src/ports.rs`, codes P001–P004 and M007).
- **Single implementation searched for:** `hop_function_name` exists but is crate-private. Nothing shared defines the `System` members. The member lists of the planner and S020 agree today (READ).
- **Change scenario:** a new composite version adds a `System` member or renames the route functions. The planner edit is necessary. The S020 lists and `packet.rs` are SILENT edits: a missed one shows up as a rustc error in the user's crate, or as a work packet naming the wrong function. `recorder.rs` is TESTED, but only under `--config=verify`.
- **Reachable today?** Yes, both defects above, in the frozen v0/v1 profiles. Fixing either changes what a published schema accepts or what a published profile emits. That choice is the architect's (Q2).
- **Cost:** a validated, approved model that does not compile, or a packet that misdirects an implementer.
- **View:** `audit_F9`
- **Mark:** DECIDED (architect, 2026-11-13): Q2, correct both defects in place at validation time; Q3 YES. Defect 2 stays INFERRED until it is reproduced (Q2).

### F10: One `Model` type serves every schema; which fields each schema version may use is gated by hand

- **Principle:** `parnas-1972-information-hiding`, Interpretation 2; `liskov-zilles-1974-abstract-data-types`, Interpretation 1.
- **Evidence:**
  - READ: one serde struct holds the fields of schemas v0, v1 and v2. The schema is a `String` (`l2r_model/src/architecture.rs:28-45`). `deny_unknown_fields` therefore applies to the union of all versions, not to each version.
  - READ: the per-version gates are written by hand at `l2r_validate/src/structure.rs:16-27, 59-71`, `ports.rs:35-41` and `v2/components.rs:20-31`.
  - READ: one was missed. Commit 05e855e: "S015: a v0/v1 model cannot use the v2 port fields direction and capacity (before this commit they were silently accepted and ignored)."
  - READ: no test pins the set of documents that v0 or v1 accepts. `l2r_validate/tests/rules.rs` has per-rule rejections only.
- **Change scenario:** schema v3 adds a field, such as the fan-out ports deferred by ADR-0005. Adding it to `SUPPORTED_SCHEMAS` and to the dispatch is TESTED. The gates for v0, v1 and v2 are SILENT: a missed one means an older document silently accepts and ignores the new field. That changes the meaning of a published schema.
- **Reachable today?** No. The gates cover today's fields (READ).
- **Cost:** each schema version adds a gate to every older version, and a miss weakens a published schema.
- **View:** `audit_F10`
- **Mark:** DECIDED (architect, 2026-11-13) under Q1: schema identity and permitted combinations get one typed owner.

### F11: Frozen profiles share output helpers with the versions still changing

- **Principle:** `parnas-1972-information-hiding`, Interpretation 2. What `composite/v0` emits is partly decided in modules that also serve v2.
- **Evidence:**
  - MEASURED: every profile module, v0/v1 and v2 alike, uses `l2r_plan::text`. `profiles::composite_v1` and `lean_composition` use `profiles::composite::edges_within`.
  - READ: `layout::component_dirs` serves both pipelines (`lib.rs:58`, `v2/mod.rs:53`).
  - READ: the freeze is guarded by pins:
    - one v0 example digest (`l2r_plan/tests/determinism.rs:153-183`);
    - one v1 fixture (`tests/composite_v1.rs:119-138`);
    - the committed example scaffolds (`l2r_check_test`).
- **Change scenario:** a v3 need changes `text::type_name` or `layout`. Within the pinned examples it is TESTED. For inputs those examples don't exercise it is SILENT. The user would see it as a changed plan digest after upgrading L2R.
- **Reachable today?** No.
- **Cost:** a silent break of "published profiles are immutable" (AGENTS.md).
- **Not a breach in part:** `text::generated_marker` and `declares_generated` are one deliberate system-level decision (READ `text.rs:14-16`).
- **View:** `audit_F11`
- **Mark:** DECIDED (architect, 2026-11-13): Q4, freeze version-dependent output semantics. This is option (a), not the recommended (b): pins and fixtures stay, but finite examples are not the guarantee.

### F12: Three mechanisms write files outside the output tree; the inventory's follows a planted symlink

- **Principle:** `saltzer-schroeder-1975-protection-principles`, economy of mechanism and fail-safe defaults.
- **Evidence:**
  - READ: the approval record goes through an exclusively created temporary and a rename, and a planted symlink is replaced, never followed (`l2r_apply/src/approval.rs:239-265`).
  - READ: `l2r init` uses `create_new` (`l2r/src/init.rs:300-309`).
  - READ: `l2r inventory` uses `fs::write` to the fixed name `<out>.json.tmp`, then renames it (`l2r/src/inventory.rs:49-51`). `fs::write` follows a symlink at that name.
  - READ: the exporter is written to the predictable path `temp_dir()/l2r-inventory-{pid}.lean` (`inventory.rs:70`).
  - INFERRED: git stores symlinks. In a cloned project with a committed `lean-inventory.json.tmp` symlink, `l2r inventory` would truncate the link's target and write JSON into it. Hostile trees are in the project's threat model (`l2r_apply/tests/hostile.rs`).
- **Reachable today?** Yes, as inferred. It was not run.
- **Scope note:** G1 covers the output tree only (`docs/generation-boundary.md:38`), so this does not breach a stated guarantee. Whether files written beside the model need the same rule is the architect's question (Q5).
- **View:** `audit_F12`
- **Mark:** DECIDED (architect, 2026-11-13): Q5, one trustworthy file-writing mechanism; each operation keeps its own authorization policy (inventory export needs no generation approval). Highest priority.

### F13: The project layout and the meaning of `check` are decided in the CLI

- **Principle:** `parnas-1972-information-hiding`, Interpretation 2; `ousterhout-deep-modules` (provisional), Interpretation 1, on an obligation stated nowhere.
- **Evidence:**
  - READ: `l2r_apply` owns the approval record's format and writer (`approval.rs:36, 242`), but its location `{stem}.approval.json` is decided in `l2r/src/session.rs:113-120`. It is restated by `l2r_apply/testing/crash_main.rs:22-24` and `bazel/l2r.bzl:21`.
  - READ: `l2r_apply::check` covers drift, the lock, budgets and a pending journal (`check.rs:32-50`). Approval presence and staleness are added only in the CLI (`l2r/src/commands.rs:184-201`), but `bazel/l2r.bzl:7-11` promises both under "`l2r check`".
  - READ: the CLI restates rule S013, the safe-path check on `[system]` paths, with its code, field list and message (`session.rs:84-104`, against `l2r_validate/src/structure.rs:91-101`). It has to, because `validate` needs the inventory, which is opened from a path in the model. The predicate is one function, `names::is_safe_relative_path`. That makes two checks of one rule, with the rule's field list written twice.
- **Change scenario:** a second front end, such as an editor plug-in or a library user, calls `l2r_apply::check` and silently loses the approval check. A new path field in `[system]` needs a SILENT edit in `session.rs`.
- **Reachable today?** No. The CLI is the only front end.
- **View:** `audit_F13`
- **Mark:** DECIDED (architect, 2026-11-13): Q6, option (b). The CLI stays the one supported front end; document its layout and `check` semantics; consolidate genuinely duplicated path rules without changing behaviour.

### F14: Guarantees carried by `Validated` are dropped inside the planner

- **Principle:** `liskov-zilles-1974-abstract-data-types`, Interpretation 1 and the Application bullet "re-checks, or silently assumes, a property established elsewhere"; `parnas-1979-uses-hierarchy`, Interpretation 5.
- **Evidence:**
  - READ: `Validated::model()` hands out the raw `&Model` (`l2r_validate/src/resolved.rs:104`). The planner then re-derives facts the validator established, with three failure policies:
    - **Return an error:** `lib.rs:42-45`.
    - **Panic:** `transition_core.rs:26`; `composite.rs:347, 375-376`; `composite_v1.rs:107`.
    - **Guess a default:**
      - `composite_v1.rs:95` falls back to capacity `unwrap_or(1)`;
      - `lean_composition.rs:261` falls back to capacity `unwrap_or(0)`;
      - `lean_composition.rs:289-292` uses `unwrap_or_default()` on a port's target index, so the port would inject into child 0.
  - READ: acyclicity of the containment tree is relied on by two walks that do not check it: `layout.rs:13` and `v2/mod.rs:67-82`.
  - READ: the hop-shape properties of `Flat` ("every hop is total", "the first is always an edge") are stated only in doc comments (`l2r_validate/src/flat.rs:55,64`). They are relied on in `l2r_plan/src/v2/system.rs:362-372` and `lean_composition_v2.rs:53-63`.
  - Contrast (READ): for v2, `Flat` carries capacity as a `u32` (`flat.rs:32,43`), and both the Rust and the Lean read it from there. The guarantee is in the type, with one owner.
- **Already in the knowledge base:** the two capacity fallbacks are the example in `parnas-1972-information-hiding` ("One decision, three owners"). `layout::component_dirs` is the example in `parnas-1979-uses-hierarchy` and `liskov-zilles-1974-abstract-data-types`. All are still at the cited lines (READ). The new evidence is the port-index default and the second tree walk.
- **Reachable today?** No. Each relied-on rule is enforced (S017, P003, the shape checks).
- **View:** `audit_F14`
- **Mark:** DECIDED (architect, 2026-11-13): improve as part of the typed version/validation seam (Q1), not separately.

### F1–F7: `l2r_apply` (carried over unchanged)

The full record is the audit posted on PR #20 (issuecomment-6850881232) and its correction. In summary:

- **F1:** the ownership half of the transition rule is restated in `Step::validate`, which runs on recovery only. `Approval::authorizes` and `Step::authorize` are single implementations used on both paths. **ARCHITECT (2026-11-13):** "My decision is to retain this as a proposed architectural finding, not authorize a refactor yet. We must preserve independent checks on normal and recovery paths while avoiding multiple definitions of the same rule." Mark: PROPOSED.
- **F2:** the state-file names have two owners: `l2r_model/src/path.rs:13` and `l2r_apply/src/lock.rs:20`, `journal.rs:31`. PROPOSED.
- **F3:** `commit` is correct only because `Journal::authorize` ran first; no type carries that. PROPOSED.
- **F4:** module loop approval → assess → journal. PROPOSED.
- **F5:** formats produced by `l2r_model` and `l2r_validate` are re-validated in `lock.rs` by independently written rules. PROPOSED.
- **F6:** module loop fault ↔ tree, a test seam. It is measured with features merged. PROPOSED.
- **F7:** the generator label is formatted in two modules. PROPOSED.

### F15: Module loop `lean_composition ↔ lean_composition_v2`

- **Principle:** `parnas-1979-uses-hierarchy`, Interpretation 3.
- **Evidence:**
  - MEASURED: the loop is in `module_loops`.
  - READ: `lean_composition.rs:69` dispatches to the v2 deriver.
  - READ: `lean_composition_v2.rs:16` imports `SEMANTICS_VERSION_V1` and `semantics_digest_v1` back.
  - READ: `conformance.rs:36-37` imports identities from both.
- **Which side splits:** `lean_composition` holds three decisions: the identities of the Lean semantics and scheduler sources (`:31-54`), the v0/v1 deriver, and the schema dispatch. Moving the identities into their own module, which both derivers and conformance use, removes the loop. The dispatch would then sit above both derivers.
- **Reachable today?** The loop exists, but it has no runtime effect. The cost is small.
- **View:** `audit_F15`
- **Mark:** DECIDED (architect, 2026-11-13): lower-priority graph clean-up.

### F16: The build does not enforce the crate layering

- **Evidence:** MEASURED: `visibility = ["//visibility:public"]` on `l2r_model`, `l2r_validate`, `l2r_plan`, `l2r_apply`, `l2r` and `l2r_testing` (`crates/*/BUILD.bazel`).
- **Consequence (INFERRED):** a new dependency against the layering compiles. Only review would catch it, for example `l2r_validate` depending on `l2r_plan` to share the names of F9.
- **Mark:** DECIDED (architect, 2026-11-13): Q7 YES; public integration entrypoints stay usable and the external new-project workflow is verified.
- **View:** `audit_overview`

## Not findings

- **Profile identifiers as constants:** each one has a single definition (READ `l2r_model/src/profiles.rs`). Other mentions are doc comments or messages.
- **Per-version copies** of validation rules (`messages.rs` vs `v2/edges.rs`, `ports.rs` vs `v2/ports.rs`), of the BUILD template (`profiles/crate_root.rs` vs `v2/crate_root.rs`) and of the provenance headers: each copy belongs to a separate frozen version. That is how immutability is kept, not a rule written twice.
- **Dispatch on profile strings in the planner** (`lib.rs:84`, `v2/mod.rs:139`): an unknown profile gives a loud `PlanError`.
- **Rust and Lean route expressions:** both `match` exhaustively on `HopKind`, so the compiler forces them to change together (FORCED), and conformance tests their agreement (TESTED).
- **The trace protocol** in `recorder.rs` and `replayer.rs`: a drift makes every sequence report MISMATCH.
- **`init` templates vs `examples/starter`:** TESTED by `verification/composition_v2/composition_test.sh:218-224` and the end-to-end lane.
- **Authority on generate and recover:** `l2r_apply` verifies the approval itself on both paths (`transaction.rs:107, 172`), so the CLI need not.
- **Inputs read through symlinks** (the model, the inventory, the approval): G1 deliberately scopes the symlink rule to the output tree.
- **Destructive permissions taken from the tree at approve time, not from the preview** (`l2r/src/commands.rs:107-109`): this needs the tree to change between `preview` and `approve` while the plan digest stays the same. It is left out as unlikely.
- **`init` workspace pins** duplicating `MODULE.bazel`: a breaking mismatch is TESTED by `tools/e2e/new_project.sh`, and the pins have not changed since 64c9a17.

## Questions for the architect

All seven are answered; see [Decided](#decided).

Each question changes the modules, the uses relation or an interface. Q1 constrains Q2 and Q3.

| Id | Finding | Question | Alternatives and consequences | Recommendation, and why |
|---|---|---|---|---|
| Q1 | F8, F10 | Will profiles and schemas keep arriving as new versions, and should one module own what each version *is*? | (a) A profile type in `l2r_model` (an enum, or a descriptor with its kind, schema and allowed children) that validate, plan and the CLI ask. A `match` on it makes every site an edit the compiler forces. **Constraint:** `Model` is digested as canonical JSON, and that digest is part of every plan digest (ADR-0003 D1, D6). The type must serialize to the same bytes as today's strings, or every existing approval goes stale. (b) Keep strings, and add a test that enumerates each profile through every classifier. That is TESTED, not FORCED. (c) Leave it; every version pays the silent edits. | (a). The decision changes with every version, and one owner hides it (`parnas-1972-information-hiding`, Interpretation 2). A type carries it to the callers (`liskov-zilles-1974-abstract-data-types`, Interpretation 1). |
| Q2 | F9 | The two reachable defects are in frozen v0/v1 profiles. Fix them in place, add new versions, or document them as limitations? | (a) A validation rule in place. **This reopens a binding decision:** ADR-0005 D9 says "`v0` and `v1` are unchanged". It would newly reject only models whose output does not compile or mirrors the wrong type. (b) New versions (`composite/v3`, …). The schemas stay pure, at the cost of more versions. (c) Document only. | No recommendation. Whether (a) is allowed is a reading of ADR-0005 D9, and that reading is yours. The argument for (a): it changes no generated byte of any model whose output is correct. |
| Q3 | F9 | Should the generated names have one owner that validate (S020), the CLI packet and conformance ask, instead of restating them? | (a) A naming module the planner uses and exports, which validate cannot import while it sits below the planner. It needs either a lower crate for "the names a profile generates", or moving S020 into the planner as a plan-time check. (b) A test that compares the S020 lists with the planner's output for each example. | (a) with the lower crate. It hides the naming scheme once and keeps validation before planning. This is the case where the design skill should lay out the alternatives (hand-over). |
| Q4 | F11 | Should a frozen profile own all the text it emits, including helpers, or are shared helpers guarded by pins enough? | (a) Freeze helpers per version. This means copies, but no frozen output can move. (b) Pins over a corpus of models that exercises every branch of every frozen profile. | (b), unless you want (a)'s certainty. Copies trade one risk for another. |
| Q5 | F12 | Should every file L2R writes outside the output tree go through one writer with the approval record's guarantees? | (a) One writer, which fixes the inventory symlink. (b) Fix `inventory.rs` alone. | (a). One mechanism per authority decision (`saltzer-schroeder-1975-protection-principles`, economy of mechanism). *Note after the decision:* the question tied the writing mechanism to the approval record's authorization; the architect separated them (one mechanism, per-operation authorization). |
| Q6 | F13 | Should the project layout (where the approval record and inventory live) and the full meaning of `check` belong to a library rather than the CLI? | (a) Move them into `l2r_apply` or a small project-layout module. (b) Keep them in the CLI and state that it is the only supported front end. | (a), if a second front end is planned. Otherwise (b), stated in the docs. |
| Q7 | F16 | Should Bazel `visibility` enforce the crate layering? | (a) Restrict each library's visibility to the crates above it. The CLI binary stays public: downstream projects run `@l2r//crates/l2r_cli:l2r` (READ `bazel/l2r.bzl:35`, `l2r/src/init.rs:174`, `tools/e2e/new_project.sh:44`). (b) Keep public. | (a). Outside `crates/`, only the CLI binary is a direct dependency (MEASURED `bazel query 'rdeps(//..., //crates/...) except //crates/...'`: the five examples' drift tests and `composition_v2_test`, all through `//crates/l2r_cli:l2r`), so restricting the libraries breaks no known user. |

F14 and F15 need no question before Q1 is answered: their remedy depends on how Q1 shapes the types that cross from validate to plan.

## Decided

The architect's decisions, quoted verbatim from the PR #21 comment of 2026-11-13. They are authoritative.

| Question / finding | Decision (ARCHITECT, 2026-11-13) | Hand-over to design? |
|---|---|---|
| Q1 (F8, F10) | "**Q1 — YES.** Schema/profile identity, classification and permitted combinations must have a single typed owner in the domain model. Preserve existing canonical model JSON, digests, approvals and all valid frozen-profile outputs byte-for-byte. The design skill may determine the abstraction and module boundaries; do not substitute a global registry of string comparisons." | yes |
| Q2 (F9) | "**Q2 — Correct the two v0/v1 defects in place at validation time.** Published immutability protects the meaning and output of valid models, not the accidental acceptance of inputs that produce uncompilable Rust or incorrectly conflate distinct Lean message types. Reject those inputs with precise diagnostics; do not silently rename generated types or rewrite existing valid artifacts. Capture this limited correction as an explicit clarification to ADR-0005 D9, not as permission for other changes to frozen profiles. Reproduce the second suspected defect before claiming it is confirmed." | no |
| Q3 (F9) | "**Q3 — YES.** Generated Rust naming needs a single owner usable by validation, planning, work packets and conformance without reversing crate dependencies. Hand that architectural seam to the design skill, then implement; prevent name conflicts before approving a plan." | yes |
| Q4 (F11) | "**Q4 — Freeze version-dependent output semantics.** Sharing truly stable infrastructure/helpers is fine, but a new version may not change even untested valid inputs' v0/v1 generated bytes. Keep regression fixtures and pin checks; do not mistake finite examples for the immutability guarantee." | no |
| Q5 (F12) | "**Q5 — YES to one trustworthy *file-writing mechanism*, NO to requiring generation approval for Lean inventory export.** Fix both the predictable inventory temporary file and predictable OS-temp Lean exporter. The shared primitive must prevent symlink-following/unsafe replacement; each operation retains its own authorization policy. This is the highest-priority safety correction. Demonstrate hostile-path refusal." | no |
| Q6 (F13) | "**Q6 — Keep the CLI as the one supported front end for now.** Document its project-layout and full `check` semantics accurately; avoid inventing a reusable front-end layer before a real second consumer exists. Consolidate genuinely duplicated path rules where this can be done without changing behavior." | no |
| Q7 (F16) | "**Q7 — YES.** Enforce intended crate dependency boundaries with Bazel visibility while keeping documented public integration entrypoints usable. Verify the external-new-project workflow." | no |
| F1 | "My decision is to retain this as a proposed architectural finding, not authorize a refactor yet. We must preserve independent checks on normal and recovery paths while avoiding multiple definitions of the same rule." | no |
| Other findings | "F1 remains PROPOSED with no authorized refactor. F14 can be improved as part of the typed version/validation seam; F15 is a lower-priority graph clean-up. No broad rewrite of the structurally sound crate architecture. The cited `ADR-0003 D8` appears invalid; verify the authoritative Discussion text and correct the citation without altering the decision's substance." | no |
| Priority | "first the reachable unsafe inventory write and invalid v1 input cases, then schema/profile/naming ownership and compatibility protection, then proportionate cleanup. Keep proof claims and frozen behavior honest." | — |

F2–F7 received no decision and stay PROPOSED.

## Knowledge base

No contradiction found. One suggestion, for the architect to decide: `parnas-1972-information-hiding` lists the shared `MAX_MAILBOX_CAPACITY` (used by v1 and v2) as not a breach. Under immutability, a change made for v2 to a limit shared with v1 alters what v1 accepts. The example may need that qualification. This is a proposal only; the approved file is unchanged.
