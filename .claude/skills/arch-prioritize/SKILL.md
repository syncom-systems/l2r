---
name: arch-prioritize
description: arch family, prioritize phase. Rank which files or functions to improve first by combining CodeScene hotspots, technical-debt goals, Code Health and change frequency. Use when asked what technical debt to tackle first, which hotspots matter, or where a Code Health campaign should start.
---

# arch-prioritize

Debt in code that never changes is cheap. Debt in code that changes every week is expensive. Rank targets by how much the low health costs right now, not by score alone.

## Steps

1. **Fix the scope.** Use the area the user named. If the work is tied to a task, the scope is the files that task touches. Never rank more than the user asked about.
2. **Gather the signals.**
   - CodeScene project data, when a project exists: `select_project`, then `list_technical_debt_hotspots_for_project` and `list_technical_debt_goals_for_project`. Use the `_file` variants to drill into a shortlisted file.
   - Change frequency from git, always: `git log --since=6.months --name-only --format= -- <scope> | sort | uniq -c | sort -rn | head -30`.
   - `code_health_score` for each candidate that has no hotspot data.
3. **Rank** in this order:
   1. files with an explicit goal;
   2. low health together with high change frequency;
   3. low health in code that the current task must change anyway;
   4. everything else (background only).

   A low score in a file with no churn and no goal goes at the bottom.
4. **Report** at most 5 targets: path, score, churn, goal (if any), and the one or two dominant findings. Add one line on why the target is worth the cost.

Stop there unless the user asked for remediation. To remediate, continue with `arch-diagnose` on the top target.
