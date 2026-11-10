---
name: arch-verify
description: arch family, verify phase. Independently judge whether a refactoring genuinely improved both Code Health and architecture, or only moved the score, using deterministic checks (CodeScene before/after per finding, complexity displacement, gaming patterns, impact level) plus a review sized to the impact. Use before accepting, committing or merging any Code Health refactor, when a CodeScene score "went up" and you need to know whether it is real, or when reviewing another agent's refactoring.
---

# arch-verify

Judge the diff, not the account of it. An implementer's "findings fixed, score 9.8" is a claim. The checks below decide.

## 1. Deterministic checks

```bash
python3 .claude/skills/arch-verify/scripts/arch_check.py --repo . --base <base-ref>
```

It is standard-library Python and read-only. It compares `<base-ref>` with the working tree (or `--head <ref>`) and reports:

- **CodeScene per file**: score before → after, plus the findings resolved, new and still present.
- **Displacement**:
  - findings that are new in any changed or added file;
  - files whose score dropped;
  - decision points (branches) that left one file and reappeared in another. Logic pushed into a caller often stays under every CodeScene threshold, so this is the only signal for it.
- **Gaming flags**:
  - placeholder helper names;
  - bag-shaped types;
  - callable-carrying fields and lambdas;
  - `**kwargs`;
  - new suppressions;
  - private helpers made public, or visibility widened to cross a split;
  - new macros;
  - files that became unscorable;
  - edited generated files;
  - edits to CodeScene rules or debt baselines;
  - single-implementation abstract types;
  - lost test assertions.
- **Impact floor**: Local, Module or Architectural, with the reasons.

Every flag needs a verdict from you: confirmed, or dismissed with a reason grounded in the code. If the script cannot reach CodeScene, record that and fall back to `code_health_review` on the base copy (`git show <base>:<path>`) and on the current file. Never skip the comparison.

## 2. Review sized to the level

Take the higher of the script's floor and the diagnosis level. Answer the questions for that level and every level below it in [references/architecture-checks.md](references/architecture-checks.md). Read the diff and the touched files. At the Local level, read no architecture documents.

**Independence.** At the Module level and above, when you also wrote the change, run this step in a fresh sub-agent. Give it the repository path, the base ref, the diagnosis and the script output, but not the implementer's explanation.

## 3. Verdict

- **PASS** requires all of the following:
  - every diagnosed finding is resolved at its cause, or reported as open debt with a named reason;
  - nothing was displaced;
  - every flag is resolved;
  - the architecture questions pass;
  - tests are green;
  - `pre_commit_code_health_safeguard` (or `analyze_change_set` for a branch) reports no regression.
- **FAIL** when any of these hold: a confirmed gaming move, displacement, a failed deletion test, weakened seam or layering, or a score that rose while the targeted cause remains. List each with its file and line and the honest alternative.
- **ESCALATE** when the change falls under the escalation list in `../arch/references/impact-levels.md` without the user's approval: a published external contract, a live-data migration, a security or authorization change, or a conflict with a binding ADR or ruling. A changed internal interface or seam is judged by the architecture questions, not escalated.

A 10.0 never decides the verdict on its own, and neither does a cohesion argument that would close a complexity finding. A finding that remains is never dismissed by an architectural argument. It stays visible as open debt. It blocks PASS only when it belongs to the agreed remediation objective. Debt that was there before or was discovered along the way and is outside that objective is reported without blocking.

On FAIL, the next round starts with `arch-diagnose` on what failed. That re-diagnosis is the one sanctioned CodeScene pass between the baseline and verification.

## Report

```
verdict: PASS|FAIL|ESCALATE        level: Local|Module|Architectural
<file>: 6.95 → 8.71   resolved: Complex Method _validate_posting_intent (cc 15), Bumpy Road ...   new: none
flags: <flag> — confirmed|dismissed: <reason>
architecture: <one line per question answered at this level>
open debt: <finding> — <reason>
```
