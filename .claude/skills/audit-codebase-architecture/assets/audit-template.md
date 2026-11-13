# Architecture audit: <scope>

Architect: <name>. Date: <date>. Knowledge base: `architecture-knowledge/` (rulings first).

Evidence labels: MEASURED (deps.py, compiler, build), READ (`file:line`), INFERRED (chain stated), ARCHITECT (answer, date).
Marks: PROPOSED · DECIDED (architect, date) · OPEN.

## Scope and evidence

- In scope: …
- Out of scope, and why: … (generated code, other subsystems)
- Binding decisions read: … (AGENTS.md, ADRs)
- Measurement: `deps.py --scope …` → exit code, crates found by …; scope …; context (used by scope …, uses scope …); M modules, crate loops …, module loops …
- Not measured: … (`not_measured`, unresolved deps, declared deps without a reference)

## Findings

Ordered by cost × likelihood of the change each one makes expensive.

### F1: <one line>

- **Principle:** <file>, <item>
- **Evidence:**
  - MEASURED: …
  - READ: `path:line` …
- **Reachable today?** …
- **Cost:** which likely change becomes expensive, and why.
- **View:** `audit_F1`
- **Mark:** PROPOSED

## Not findings

Candidates set aside, with the reason (a principle's "Not a breach", or code that proves the candidate wrong).

## Questions for the architect

| Id | Finding | Question | Alternatives and consequences | Recommendation, and why |
|---|---|---|---|---|
| Q1 | F1 | | | |

## Decided

| Finding | Decision | Architect, date | Hand-over to design? |
|---|---|---|---|
