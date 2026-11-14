---
name: audit-codebase-architecture
description: Audit the architecture of code that already exists, for the architect to decide what to change. Measures the real crate and module dependencies with file:line evidence, finds decisions known by more than one module, loops in the uses relation, guarantees relied on but not carried by types, shallow interfaces and authority checks with gaps, all judged against the project's architecture knowledge base, then stops with findings and questions for the architect. Use whenever someone wants to audit, review or map the architecture of existing code, asks why a crate or subsystem is hard to change, or wants to know what to restructure before refactoring. For something not yet built, use design-codebase-architecture.
---

# Audit codebase architecture

Find where existing code makes a decision in more than one place, relies on what it does not state, or depends the wrong way. The audit shows the evidence and proposes; the architect decides what to change (ruling R2). It changes no code.

## Before anything else

1. Read `architecture-knowledge/rulings.md` at the repository root. The rulings outrank this skill. If there is no `architecture-knowledge/`, ask the architect where the knowledge base is. Do not proceed from memory.
2. Read `architecture-knowledge/README.md`. Read a principle file when its "Read it when" matches what you are looking at. Each file's **Application** section says how a breach shows in code and what is *not* a breach. Use those sections to confirm or dismiss a candidate.
3. Read what already binds the code: `AGENTS.md` or `CLAUDE.md`, accepted ADRs (wherever the project keeps them; the agent instructions say where), design documents. Name any you could not read. A finding that would reopen a binding decision says so, and the decision stays the architect's.
4. Find the generated code and leave it out of the audit. It is judged through its generator.

Your sense of what is usual structure is not evidence (R3). Every fact in the audit carries one label:

- **MEASURED:** from the `architecture-evidence` measurement, the compiler or the build graph.
- **READ:** a `file:line` you looked at.
- **INFERRED:** reasoning from measured or read facts, with the chain stated.
- **ARCHITECT:** the architect's answer, with the date.

## The audit record

Everything goes into `architecture/audit.md`. Start it from [assets/audit-template.md](assets/audit-template.md). Each finding gets:
- an id (F1, F2, …);
- the principle it breaches, cited by file and item;
- the labelled evidence;
- its cost, meaning which likely change it makes expensive;
- a mark: PROPOSED, DECIDED (architect, date) or OPEN.

The picture is the measured model plus one view per finding (ruling R4); see [Pictures](#pictures).

## Steps

### 1. Scope and measurement

Name the scope (crates, packages or a subsystem) and what is outside it. A new audit replaces the previous one: its record, its views and everything in `architecture/audit/` (git keeps the old ones). Remove what the previous audit left there, then measure with the `architecture-evidence` skill, writing to `architecture/audit`:

```bash
python3 <skills>/architecture-evidence/scripts/measure.py --repo . --scope <dirs> [--exclude '<generated glob>'] --out architecture/audit
```

Follow that skill on the exit code and on what the evidence does not show. Do not audit without a measurement. In short:
- `evidence.json` lists the build units of the scope and its context (`used_by_scope`, `uses_scope`), their source units, and **declared** dependencies (from the build) and **observed** dependencies (from the source), each with `file:line`, plus their loops and checks. Context units are not audited, but the dependencies crossing the scope boundary are measured.
- Name in the record what `not_measured`, `unresolved_deps` and `checks.declared_without_observed` leave out.

Every dependency is a compile-time fact. Whether it is a *use* in Parnas's sense (A's correctness depends on B) is your judgement. A dependency on a type alone usually is one, but a weak one; say which.

For a language the evidence skill does not measure, collect the same facts from the build files and imports, and label them READ.

### 2. Decisions known in more than one place

This is the finding that metrics rarely show and that matters most (`parnas-1972-information-hiding`).

1. List the decisions this code makes that are difficult or likely to change: formats, limits, naming schemes, orderings, protocols, policies, file names, external systems.
2. For each decision, find every module that knows it. Search for the constant, the message, the expression or the rule, and read each hit.
3. More than one owner is a finding, unless the decision is deliberately system-level and defined once (Not a breach). If the copies can disagree, say whether they do, and whether that difference can ever be reached.
4. Several *checks* of one rule are not several owners. Checking a rule on every path is what complete mediation asks for (`saltzer-schroeder-1975-protection-principles`). The breach is the rule *written* several times, so that the checks can drift apart. Report which of the two you found.
5. Before recording a duplication, look for an existing single implementation of the rule and check whether every path calls it. Then name one concrete, likely change to the decision and list every edit it forces. For each edit, say whether the compiler forces it (an exhaustive `match`, a type that no longer fits) or whether it can be missed silently. Only edits that can be missed silently, and that would make two paths disagree, support the finding.

Include the decisions about **authority**: what may be written, overwritten, deleted or sent. For each, find every path that reaches the protected action, including recovery and retries, and check that each one passes the same check (`saltzer-schroeder-1975-protection-principles`).

### 3. The uses relation

From `evidence.json` and reading (`parnas-1979-uses-hierarchy`):

- **Loops.** For each loop, read the edges. Name which side would split, and into what.
- **Reliance without a call.** A module that is correct only because something else ran first: validation, sorting, acyclicity, authorization. Check whether a type carries that guarantee (`liskov-zilles-1974-abstract-data-types`). If only the call order carries it, that is a finding. If a type carries it at a boundary and an inner function drops it, report exactly that and no more.
- **Direction.** A lower module that names a higher module's types or concepts.

### 4. Interfaces

Look at the interfaces that the most modules use, and the widest ones (`ousterhout-deep-modules`, `liskov-zilles-1974-abstract-data-types`):

- the representation exposed;
- obligations that are stated nowhere;
- pass-through layers;
- configuration the module could decide for itself.

Apply the deletion test: if the module were deleted, would complexity reappear in its callers, or vanish?

### 5. Confirm before recording

For every candidate:
- read the code that would prove it wrong;
- check the principle's "Not a breach" list.

A finding claims only what you read. If the defect cannot be reached today, say so plainly, and say what change would make it reachable.

### 6. Hand the findings to the architect

Stop and present the findings.

1. Order them by the cost of the change each one makes expensive, multiplied by how likely that change is. Without the architect, judge likelihood from evidence, labelled INFERRED: the change history, earlier versions of a format, comments and documents that announce a change.
2. For each finding, give:
   - the evidence and its view;
   - the question only the architect can answer, for example whether this is intended, or whether this decision will change. Fold follow-up questions into the alternatives;
   - the alternatives with their consequences, and your recommendation with the principle behind it.
3. Record the answers as DECIDED with the date.

A remedy is a design question. When the architect wants one designed, hand it to **design-codebase-architecture**:
- the audit's facts become its **Given** entries;
- the decisions to re-home become its **Decisions to hide**.

If the code contradicts an example in the knowledge base, report it to the architect as a proposed correction to that file. Do not edit an approved file yourself.

## Pictures

`measured.c4` is generated; never edit it. Copy [assets/spec.c4](assets/spec.c4) to `architecture/spec.c4` if it is missing. In `architecture/audit.c4`, write views only, one per finding, using the measured elements. Do not write model elements:

```c4
views {
  view audit_F1 {
    title 'F1: <one line>'
    include measured.<crate>.<crate>__<module>, measured.<crate>.<crate>__<other_module>
  }
}
```

Element ids are the module path with `::` replaced by `__`, nested under the crate (`measured.my_crate.my_crate__store__index` for `my_crate::store::index`). Run `likec4 validate architecture/` after each edit. Offer `likec4 start architecture/` when the architect is present.

## Rules

- Change no code, build files or tests. Write only under `architecture/`.
- Never mark anything DECIDED without the architect's answer. There is no unattended mode. Without an architect, the audit ends with PROPOSED findings and its questions.
- Do not restate the knowledge base in the record. Cite the file and the item: an Interpretation number, or an Application bullet.
- Do not impose a layering the code does not have. Propose levels from the uses relation, and let the architect confirm them.
