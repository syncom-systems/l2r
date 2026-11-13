# Agent skills for working on L2R

These are instructions for coding agents (Claude Code loads them from this
directory). They are not part of L2R the tool and nothing in the build depends
on them.

## Architecture knowledge

The architectural principles (Parnas, Liskov and Zilles, Saltzer and
Schroeder, Ousterhout) and the architect's rulings live in
[`/architecture-knowledge`](../../architecture-knowledge/README.md), outside any
skill. Skills refer to those files; they do not restate them. The rulings
outrank the skills.

## `arch`: code health and architecture together

CodeScene Code Health is the measurement, architectural judgement decides, and
verification is independent of the implementer.

| Skill | Phase |
| --- | --- |
| `arch` | Hub: principles, phases, impact levels |
| `arch-prioritize` | Rank what to improve first (hotspots, goals, churn) |
| `arch-diagnose` | Trace findings to design causes; the smell / honest fix / gaming move table |
| `arch-design` | One remediation per cause, with acceptance criteria |
| `arch-refactor` | Behaviour-preserving steps; no intermediate CodeScene runs |
| `arch-verify` | `scripts/arch_check.py` (per-finding delta, displacement, gaming flags, impact floor) plus level-sized architecture checks; PASS / FAIL / ESCALATE |
| `codebase-design` | The shared design vocabulary (module, interface, depth, seam) |

CodeScene is reached through its MCP server, configured in `/.mcp.json`. It
needs a CodeScene sign-in (the server's `login` tool) or `CS_ACCESS_TOKEN`.

What the code-health gate judges is set in `/.codescene/code-health-rules.json`:
generated scaffolds and derived files are produced by the generator and are
judged through it, not as handwritten code. Changing that file changes the
measurement and is a maintainer decision.

## Third-party notice

`codebase-design` is adapted from Matt Pocock's `skills` repository
(https://github.com/mattpocock/skills), MIT licensed.
