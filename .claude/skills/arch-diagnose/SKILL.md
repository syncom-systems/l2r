---
name: arch-diagnose
description: arch family, diagnose phase. Turn CodeScene Code Health findings for a file into design causes, separate essential from accidental complexity, and assign an impact level. Use when a file or function has CodeScene findings (Complex Method, Large Method, Brain Method, Low Cohesion, Excess Arguments, Duplication, Bumpy Road, Primitive Obsession, ...) and you need to understand them before refactoring.
---

# arch-diagnose

A finding is a symptom. Findings cluster: a Brain Method, Excess Arguments and String Heavy Arguments in a Low Cohesion file are usually one missing concept, not four problems. Diagnose causes, not counts.

## Steps

1. **Baseline.** Run `code_health_review` on the target and keep the full output (score and every finding with its function). This is the "before" that verification compares against. Do not re-run it to see whether the findings are still there.
2. **Read the flagged code**, plus enough of its callers to see how it is used. Read nothing else yet.
3. **Group the findings by cause.** For each group, answer:
   - What concept is missing or smeared? Name it in the domain's language (in this repository: the terms of `docs/model-reference.md` and the profile documents).
   - Is the complexity **essential** (the domain really has these cases) or **accidental** (the same decision made in several places, mixed abstraction levels, a missing type)? An "essential" claim must name the domain fact that forces it.
   - What move fixes the cause? See [references/smells.md](references/smells.md) for each smell's usual causes, the honest fix, and the gaming move to avoid.
   - Does the code look the way it does because shared infrastructure lacks a capability? Comments that explain a workaround ("constructed inline because the container cannot…") are the usual tell. If so, the root cause is that capability, and the move includes adding it there (see the hub's "Fix a missing capability where it lives").
   - What is the impact level of that move? See `../arch/references/impact-levels.md`.
4. **Write the diagnosis** for the next phase, one row per cause:

   ```
   cause: <missing concept / mixed responsibilities / ...>   level: Local|Module|Architectural
   findings: Complex Method _validate_posting_intent (cc 15); Bumpy Road _validate_posting_intent (2 bumps)
   move: <the remediation in one line>
   ```

At the Local level with an obvious cause, the move line is the design. Go straight to `arch-refactor`. Otherwise continue with `arch-design`.
