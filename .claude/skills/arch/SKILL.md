---
name: arch
description: Hub for improving the quality and architecture of existing code together, using CodeScene Code Health as measurement and architectural judgement as the decider. Use when the user wants to fix CodeScene findings or raise a Code Health score, clean up a hotspot or a "too complex" / "too long" / "hard to change" function or module, pick what technical debt to tackle first, or verify that a refactoring really improved the code rather than gaming the metric. Routes to arch-prioritize, arch-diagnose, arch-design, arch-refactor and arch-verify, running only the phases the task needs.
---

# arch: improve code quality and architecture together

CodeScene measures where code is hard to change. Engineering judgement decides how to fix it. Each needs the other:

- **A score is a measurement, never the goal.** A 10.0 reached by scattering logic, inventing abstractions or weakening seams is a regression. The score alone never shows a task is done.
- **Architecture can veto a fix, never a finding.** If the honest fix for a long or complex function would harm the design, the finding stays open as reported debt. "It is cohesive" does not close a complexity finding.
- **Verification is independent.** The implementer's account is a claim. `arch-verify` checks the diff itself.
- **Measure at the ends, design in the middle.** CodeScene runs at diagnosis (the baseline) and at verification (the comparison), not while refactoring. If the implementer sees the score after every step, it starts optimising the number instead of the design. This deliberately differs from CodeScene's own "re-measure after each significant change" guidance.
- **Fix a missing capability where it lives.** When the honest fix needs something shared infrastructure does not provide (a DI container that cannot resolve within a scope, a base class that lacks an extension point), fix the infrastructure. Do not work around it with inline construction, wrappers or special cases. That kind of workaround can raise the score while making the architecture worse. The extension must be causally necessary for a diagnosed cause. It is not licence for unrelated infrastructure redesign.
- **Internal architecture is ours to change; binding decisions are not.** Moving responsibilities and reshaping internal interfaces and seams is the work itself, so it proceeds without asking. A change that would contradict an accepted ADR, or a ruling recorded in the repository, is escalated rather than silently overturned. See [references/impact-levels.md](references/impact-levels.md).

## Phases

| Phase | Spoke | Produces | CodeScene |
|---|---|---|---|
| Prioritize | `arch-prioritize` | A ranked shortlist of targets, with evidence | hotspots, goals, scores |
| Diagnose | `arch-diagnose` | Findings, each traced to a design cause, with an impact level | baseline `code_health_review`, once |
| Design | `arch-design` | One remediation per cause, named in domain and design vocabulary, with acceptance criteria | none |
| Refactor | `arch-refactor` | Behaviour-preserving steps, done when the design's acceptance criteria hold | none; tests only |
| Verify | `arch-verify` | A PASS / FAIL / ESCALATE verdict from deterministic checks plus a review sized to the impact | the before/after comparison, once |

A further CodeScene pass in the middle is justified only by a meaningful redesign (the design changed while refactoring, so the diagnosis is stale and must be redone) or by a FAIL from `arch-verify` (re-diagnose what failed). It is never a progress check.

Run only the phases the task needs:

| Task | Phases |
|---|---|
| "What should we improve first?" | prioritize (then stop, or continue for the chosen target) |
| "Fix the findings in X" / "clean up function Y" | diagnose → design → refactor → verify |
| A Local fix whose cause is obvious | diagnose → refactor → verify (the design is one line in the diagnosis) |
| "Did this refactor really improve things?" | verify only |

## Proportionality

Every phase scales with the **impact level** of the change. Decide the level from the diff (or the planned diff), never from how confident you feel. See [references/impact-levels.md](references/impact-levels.md). At the Local level, read no architecture documents at all. Load the heavier references only when the level demands them.

## Shared sources (read when a spoke tells you to; do not restate them)

- `codebase-design`: the design vocabulary (module, interface, depth, seam, adapter, leverage, locality), the deletion test, `DEEPENING.md`, `DESIGN-IT-TWICE.md`.
- `architecture-knowledge/` in this repository: the architect's rulings (they outrank these skills) and the principle files, with what each source says kept apart from our interpretation. Read the file for the principle at stake; do not restate it here.
- `docs/` in this repository: the accepted design. The PRD and ADRs are GitHub Discussions; an accepted ADR is a binding decision.
- **This repository (L2R)**: Lean is the authority for behaviour, generation profiles are immutable (a refactor of a planner must leave every generated byte unchanged; the drift tests and pinned plan digests check this), and generated or derived artifacts (`examples/*/scaffold/**`, `examples/*/conformance/**`, `examples/*/composition/**`) are judged through their generator, not as handwritten code.

## CodeScene access

Use the CodeScene MCP tools (`code_health_review`, `code_health_score`, `pre_commit_code_health_safeguard`, `analyze_change_set`, `list_technical_debt_hotspots_for_project`, `list_technical_debt_goals_for_project`). `arch-verify/scripts/arch_check.py` calls the same server from a script, so verification does not rely on the implementing agent's own tool calls. Never change CodeScene rules, thresholds or suppressions to make a finding disappear; that is a decision for the user.
