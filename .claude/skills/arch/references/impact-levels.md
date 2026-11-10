# Impact levels

The level sets how much diagnosis, design and verification a change deserves. Classify it from the diff or the planned diff. `arch-verify/scripts/arch_check.py` computes a floor mechanically. You may raise the level, never lower it.

| Level | The change | Design effort | Verification |
|---|---|---|---|
| **Local** | Inside existing files of one module. No public symbol is added, removed or re-signatured. No new files. | A sentence per finding in the diagnosis. | `arch_check.py`, tests, and the Local questions in `arch-verify/references/architecture-checks.md`. Read no architecture documents. |
| **Module** | New or deleted files, or responsibilities moved between files, inside one package. Internal signatures change. | Name each new module and its interface. Apply the deletion test. | All of Local, plus the Module questions. Read the `codebase-design` glossary. |
| **Architectural** | A public or abstract interface changes, an API, schema or wire contract changes, a seam is added or removed, or code moves across packages, layers or bounded contexts. | `codebase-design` in full, `DEEPENING.md`, and `DESIGN-IT-TWICE.md` when more than one shape is plausible. | All of Module, plus the Architectural questions and the repository's design documents (`docs/`). |

The level sets effort and review depth. It does not decide whether you may make the change. Internal architectural change proceeds without asking at every level. That covers abstract interfaces inside the codebase, seams, moving code across packages, and fixing a missing capability in shared infrastructure.

## Escalate to the user before implementing only when

- the change alters a **published external contract** beyond what the task approved: an OpenAPI or other API specification that clients consume, a wire format, or a `.proto`;
- the change needs a **migration of live persisted data**;
- the change alters **security or authorization behaviour**: authentication, authority, signing, tokens, trust anchors;
- the change would **contradict a binding architectural decision**: an accepted ADR, or a ruling recorded in the repository. This is about authority, not size. A small internal change that overturns an ADR escalates. A large internal change that conforms to one does not.

Everything else proceeds. Note the choice in one line in the diagnosis or design.

## Signals the floor uses (mechanical)

- **Architectural**: changed contract files (`openapi*.y*ml`, migration directories, `.proto`); a changed or removed public signature in a file that defines an abstract interface; changes spanning more than one top-level package.
- **Module**: added, deleted or renamed source files; a changed or removed public (non-underscore) signature.
- **Local**: none of the above.

The floor is a minimum. If a Local-looking diff moves a business rule from one owner to another, it is Module or higher.
