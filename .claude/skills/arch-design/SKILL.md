---
name: arch-design
description: arch family, design phase. Choose the remediation for each diagnosed Code Health cause, naming the resulting modules and interfaces in domain and deep-module vocabulary, sized to the impact level. Use after arch-diagnose when a fix introduces or reshapes modules, moves responsibilities, or touches an interface, or when deciding how to resolve a cluster of CodeScene findings without gaming the metric.
---

# arch-design

Decide the shape before moving code, so the refactor fixes the cause instead of chasing the score. Effort scales with the impact level from the diagnosis.

## By level

**Local.** No separate design step. The diagnosis move line is the design. Check only that every new function or type has a domain name you can state without "and".

**Module.** Load the `codebase-design` glossary (its SKILL.md). For each new or reshaped module, write:

```
module: <name in domain language>      owns: <the rule or decision it owns>
interface: <what callers must know: parameters, invariants, error modes>
deletion test: <what complexity would reappear, and where, if it were deleted>
resolves: <the findings it removes>
```

A module that fails the deletion test (deleting it would make complexity vanish, not reappear) is a pass-through. Drop it. A new abstract interface with only one adapter is a hypothetical seam.

**Architectural.** Also load `codebase-design/DEEPENING.md` to classify dependencies and choose the testing strategy. When more than one interface shape is plausible, use `codebase-design/DESIGN-IT-TWICE.md`. Read `docs/` for the layering and the generation boundary. Check the design against the ADRs that govern the area. Reshaping internal interfaces and seams proceeds. Stop and present the design only for the cases listed under "Escalate" in `../arch/references/impact-levels.md`: a published external contract, a live-data migration, a security or authorization change, or a conflict with a binding decision.

**Missing capabilities.** When the diagnosis traces a cause to shared infrastructure, the design includes the infrastructure change. Name the capability, its owner module, and the workaround it removes. Keep it to what the diagnosed cause needs.

## Check the design against the finding

For each finding in the diagnosis, the design must name the move that resolves it at its cause. If the only available fix would harm the design, say so. The finding stays open as reported debt; do not invent a weaker fix to make it disappear.

## Acceptance criteria

End the design with the criteria that make the implementation complete. They are checkable statements about the code, not about the score. Examples:

- "`create_app` no longer branches on the backend";
- "every Postgres-only binding is registered by the Postgres backend";
- "no SCOPED collaborator is constructed inline".

`arch-refactor` stops when these hold. Keep the **agreed remediation objective** (the causes this design resolves) distinct from other Code Health debt in the touched files, whether that debt was already there or turned up along the way. List that other debt as the next round's candidates; do not fold it into this design unannounced. A defect such as a failing test, a type error or a bug is not debt; fix it when you meet it.
