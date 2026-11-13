# Architecture knowledge

What good architecture is, taken from a small set of primary sources, for agents (and people) designing or reviewing code. It is not part of L2R the tool, and nothing in the build depends on it. It is plain Markdown so any agent or tool can read it. Skills and agent instructions point here; they do not copy it.

Agents are trained mostly on ordinary code, so an agent's sense of what is "normal" structure is not evidence. Argue from the code, from these files, or from the architect's rulings.

## Authority

1. [`rulings.md`](rulings.md): the architect's rulings. They outrank everything else here.
2. Principle files with `status: approved`: the canon.
3. Principle files with `status: provisional` or `draft`: usable, with the limits each file states.

## Index

| File | Principle | Read it when | Status |
|---|---|---|---|
| [parnas-1972-information-hiding](principles/parnas-1972-information-hiding.md) | Decompose by the design decisions each module hides, not by processing steps | Deciding what the modules are; judging an interface; a decision found in more than one place | approved |
| [parnas-1979-uses-hierarchy](principles/parnas-1979-uses-hierarchy.md) | Design the "uses" relation; keep it loop-free so each level is a usable subset | Deciding which module may rely on which; layering; a mutual dependency | approved |
| [liskov-zilles-1974-abstract-data-types](principles/liskov-zilles-1974-abstract-data-types.md) | A type is defined by its operations; its representation is inaccessible outside them | Deciding what a type exposes; carrying a guarantee in a type; splitting a correctness argument | draft |
| [saltzer-schroeder-1975-protection-principles](principles/saltzer-schroeder-1975-protection-principles.md) | Design principles for mechanisms that guard authority: economy of mechanism, fail-safe defaults, complete mediation, least privilege, … | Designing any boundary that decides what may be read, written, executed or overwritten | draft |
| [ousterhout-deep-modules](principles/ousterhout-deep-modules.md) | A module's interface should be much simpler than what it provides | Judging an interface; deciding whether a module earns its existence | provisional |

## How a principle file is written

Each file keeps four things apart, because they carry different authority:

- **Source**: what the author says. Quotes are verbatim and checked against the copy named in the front matter, with the section. Anything not quoted is marked *(paraphrase)*.
- **Interpretation**: what we take the source to require of a design today. This is ours, not the author's.
- **Application**: questions to ask during design, how a breach shows in code, look-alikes that are fine, and examples (the source's own, and one from this repository where there is a real one).
- **Rulings**: the architect's rulings that refine the principle, by number. Only the architect adds these.

Front matter: `id`, `title`, `source` (citation and the copy used), `checked` (what the claims were checked against), `status`, `related`. A file is as long as its source warrants; sections with nothing true to say are left out.

## Sources to consider next

The architect decides what enters the canon. Keep it small.

- Parnas, Clements and Weiss, "The Modular Structure of Complex Systems" (1985): the module guide.
- Parnas, "A Technique for Software Module Specification with Examples" (1972).
- Dijkstra, "The Structure of the 'THE'-Multiprogramming System" (1968).
