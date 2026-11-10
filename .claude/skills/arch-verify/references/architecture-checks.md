# Architecture checks by impact level

Answer the questions for your level and every level below it. Each answer is one line grounded in the diff. "No" or "unclear" on any question is a FAIL or an ESCALATE unless you can justify it from the code. The terms come from `codebase-design`.

## Local

1. **Named steps.** Can every new function or type be named in domain terms, without "and" and without a placeholder (`helper`, `part2`, `do_rest`, `process_data`)?
2. **Fewer concepts to hold.** Does a reader of the original function now need fewer facts in mind? If they have to jump between pieces that only make sense together, the code was cut up, not simplified.
3. **No displacement.** Did complexity stay out of callers, tests, configuration and decorators? Callers should not need new knowledge, ordering rules or error handling.
4. **Behaviour preserved.** Are the tests unchanged in intent, or only extended? Changing an assertion to make it pass is a behaviour change.

## Module

5. **Deletion test.** For each new module, would deleting it make complexity reappear across callers? If the complexity would simply vanish, the module is a pass-through.
6. **Depth.** Is each new interface small relative to what it hides? A parameter-bag type, or a class whose methods mirror the fields it wraps, is shallow.
7. **Cohesion.** Do the pieces that were split apart change for different reasons? If they will always change together, the split is arbitrary. `git log` on the original file shows which parts have historically changed together.
8. **Locality.** Is each business rule still owned in one place? Count the files someone must edit to change the rule, before and after.
9. **Seams.** Does each new abstract type have at least two adapters (production plus a test double counts), or is it required by the repository's standards? Otherwise it is speculative indirection.

## Architectural

10. **Contracts.** Are published external contracts (API specifications, wire formats, persisted schemas of live data) unchanged, or changed with the user's approval? Changed internal interfaces are judged by questions 5–9 and 11–12.
11. **Layering.** Do dependencies still point the allowed way? In this repository: `l2r_model` ← `l2r_validate` ← `l2r_plan` ← `l2r_apply` / `l2r_cli`; a planner never reads the file system and never depends on the CLI.
12. **Boundaries.** Did any responsibility cross a package or bounded context? If so, does it now sit with its rightful owner, as the design states, rather than in a convenient place to park code? Does it conform to the ADRs that govern those boundaries?
13. **Root-cause infrastructure.** If shared infrastructure changed, is each change causally needed by a diagnosed cause, and is the workaround it replaces deleted? An infrastructure change with no workaround removed, or a workaround kept beside its fix, is a FAIL.
13. **Tests at the interface.** Do the tests exercise the new seam the way callers do? See `codebase-design/DEEPENING.md` on replacing tests rather than layering them.
