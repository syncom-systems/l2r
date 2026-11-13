# L2R: agent instructions (all coding agents)

Rust and Lean 4, built with Bazel only (ADR-0002). Lean owns behaviour, the architecture model owns structure, Rust implements. Start with `README.md` and `docs/getting-started.md`.

## Rules

- **Bazel only.** `bazel test //...`, `bazel build --config=lint //...`, `bazel build --config=fmt //...`. Changes to Lean or proofs also need `bazel test --config=verify //...`. No Cargo build.
- **Never hand-edit generated or derived files**: `scaffold/**` except `imp.rs` and `routes.rs`, derived Lean and conformance files, `lean-inventory.json`. Change the model or the Lean, then preview, approve and generate.
- **Never weaken a proof or overstate coverage.** Label every claim proved, tested or assumed (`docs/verification.md`). The v2 scheduler and data-carrying machines are tested, not proved.
- **Published schemas and profiles are immutable.** New behaviour is a new version.
- **Governance.** Never merge a PR or mark an ADR Accepted without the owner's explicit say-so; green CI is not ratification. PRDs and ADRs live in GitHub Discussions. Keep the repo generic: no internal project or customer names.

## Architecture

- Before designing or restructuring code, read `architecture-knowledge/README.md`: the architect's rulings and the principle files. The rulings outrank everything else; an agent's sense of "usual practice" is not evidence.
- Accepted decisions are the ADRs listed under "Documentation and decisions" in `README.md` (GitHub Discussions, category ADRs). They bind; a change that contradicts one goes to the owner.

## Code health

- For any CodeScene finding, hotspot or refactor, use the `arch` skills in `.claude/skills` (entry point: `arch`). A score is a measurement; architecture decides. Measure at the start and the end, verify with `arch-verify`, and never refactor only to move a number.
- CodeScene MCP is in `.mcp.json` (project 87423). Run `analyze_change_set` before opening a PR and `pre_commit_code_health_safeguard` before committing. The PR gate needs 10.0 on every new file.
- `.codescene/code-health-rules.json` excludes generated and derived files only. Don't change it, add suppressions, or lower thresholds without the owner's approval.
