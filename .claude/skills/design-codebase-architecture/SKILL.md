---
name: design-codebase-architecture
description: Prepare the architecture of something new (a system, service, subsystem, crate or module) for the architect to decide, before implementation. Builds a decision inventory, proposes modules that each hide one decision, a loop-free uses relation, and interfaces written as signatures whose guarantees are carried by types, all argued from the project's architecture knowledge base, then stops with questions and alternatives for the architect. Use whenever someone wants to design, plan or lay out new code, split a new feature into modules or layers, decide interfaces before coding, or stress-test a planned structure. For code that already exists, use an audit instead.
---

# Design codebase architecture

Design is where the quality of the code is decided. This skill prepares that design so the architect can decide it. It does not decide on the architect's behalf.

## Before anything else

1. Read `architecture-knowledge/rulings.md` at the repository root. The rulings outrank this skill and everything else. If there is no `architecture-knowledge/`, ask the architect where the knowledge base is. Do not proceed from memory.
2. Read `architecture-knowledge/README.md`. Its index says, for each principle, when to read it. Read a principle file when its "Read it when" matches the decision in front of you, not all of them up front.
3. Read what already binds the new design: accepted ADRs, documented rules, and existing interfaces it must use.

Your sense of what is usual structure is not evidence (ruling R3). Support every proposal with one of these:

- a principle, cited by file and interpretation item (for example `parnas-1972-information-hiding`, Interpretation 2);
- a ruling;
- something in the code or the brief, cited by path or quote.

If none of these supports a proposal, it is a question for the architect, not a proposal.

## The design record

Everything goes into one file, `architecture/design.md`. Start it from [assets/design-template.md](assets/design-template.md). Each row or interface section carries one mark:

- **DECIDED (architect, date)**: the architect said so. Only the architect's answer makes something DECIDED.
- **PROPOSED**: yours, with the principle or evidence it rests on.
- **OPEN**: a question the architect has not answered.
- **GIVEN (source)**: a fact from the brief, an ADR or existing code, quoted with its source. Not a decision of this design.

The LikeC4 model in `architecture/design.c4` is a picture of the record, for the architect and others to look at (ruling R4). Every relation in it names the record entry it comes from. The record is the authority; the diagram never holds a decision the record lacks. LikeC4 mechanics are in [references/likec4.md](references/likec4.md).

## Steps

Go back to an earlier step whenever an answer reopens it.

### 1. Decision inventory

Before naming any module, list the decisions this system must make that are difficult or likely to change. Typical ones are formats, representations, devices and platforms, external systems, algorithms, orderings, limits and policies (`parnas-1972-information-hiding`).

- For each decision, say why it is difficult or why it may change, citing the brief where it says so.
- Add the decisions about **authority**: what may be written, overwritten, deleted, sent or executed, and by whom. When the system has any, read `saltzer-schroeder-1975-protection-principles`.
- A decision every module must share, such as which protocol the system speaks, is hidden in no module. Mark it system-level and record it once (`parnas-1972-information-hiding`, Not a breach).
- Ask the architect which decisions are missing and which are wrong. The inventory is the foundation; get it confirmed first. When the architect cannot be asked now, continue through all steps with the inventory as PROPOSED, and make confirming it the first question.

### 2. Modules

Propose one module per decision, or per small group of decisions that always change together. A module is named by what it hides.

- **Reject decompositions by processing step.** A reader, parser, validator and executor that share a format leak that format into every stage.
- **Change scenarios.** For each likely change in the inventory, list the modules it would touch. More than one is a finding: propose merging the modules or moving the decision into one of them, or say why the spread is accepted. The architect decides which.
- **Vocabulary.** For each domain noun, name the module that owns it. When a lower module would have to use a higher module's word, the boundary is in the wrong place.

### 3. The uses relation

Read `parnas-1979-uses-hierarchy`. For each pair where one module relies on another being correct, propose the edge and justify it with criteria a–d. Record two kinds of reliance as edges, with the reliance named in the edge's row:

- reliance that involves no call, such as relying on an earlier validation;
- reliance on the order in which things happen.

- The relation must be free of loops. Resolve a mutual dependency by splitting one side, and propose the split.
- The levels follow from the relation. For each level, state what the system can still do without the levels above it. That subset is a requirement the architect confirms.
- Draw the relation in `design.c4` and run the check below. Systems outside the design (another team's module, a device) are elements of kind `external` outside the `designed` system.

### 4. Interfaces

For each module, write its interface as code signatures in the project's language: types, functions and errors. Then write down every guarantee and obligation those signatures do not show:

- call order;
- preconditions;
- side effects;
- error behaviour.

- **Carry guarantees in types** (`liskov-zilles-1974-abstract-data-types`). When a module relies on a property (validated, acyclic, bounded, authorized), propose a type that only the establishing module can construct, and make the relying module accept only that type. A precondition left as a comment is weaker. Record it as such, with the reason no type carries it.
- **Depth** (`ousterhout-deep-modules`). Compare what a caller must know with what the module does. Name pass-through layers and configuration that the module could decide for itself.
- **Design it twice** for every interface that more than one module uses or that hides a volatile decision. Sketch two genuinely different interfaces. Compare what the caller must know, how many calls one intent takes, and what stays hidden. Put both in the record for the architect.

### 5. Hand the design to the architect

Stop and present the design. Do not continue into implementation.

- The questions, in the order they must be decided. Earlier answers constrain later ones. There is no minimum or fixed number. Ask only where the answer changes the modules, the uses relation or an interface; settle the rest as PROPOSED with its reason. Fold questions that depend on one answer into that question, as its alternatives.
- For each question, the alternatives and their consequences, with your recommendation and the principle or evidence behind it.
- What each answer would unblock.

Record the architect's answers as DECIDED with the date, then update the record and the diagram. Repeat until the architect says the design is ready. A module blocked by an OPEN question that touches it is not ready.

## Checking the diagram

```bash
python3 <skill>/scripts/c4check.py architecture/ --record architecture/design.md
```

It validates the LikeC4 model and reports:

- loops in the uses relation;
- the levels that follow from the relation;
- relations that name no record entry;
- relations that name an entry the record does not contain.

It checks that the picture agrees with the record. It does not check that the design is good. That judgement is the architect's.

## When the design is ready

Propose how to keep the code true to it. The architect chooses among these:

- Write the public types and signatures first, and review them against the record before writing the internals.
- Give whoever implements a module that module's record entries, not the whole design.
- Make the build enforce the uses relation: Bazel `deps` and `visibility`, or crate dependencies, matching the record.
- Use language privacy for every representation the record says is hidden.

## Rules

- Write only `architecture/design.md` and `architecture/design.c4` (plus `architecture/spec.c4` from [assets/spec.c4](assets/spec.c4) if it is missing). Write no implementation code.
- Never mark anything DECIDED without the architect's answer. There is no unattended mode. Without an architect to ask, the output is a design with PROPOSED and OPEN entries and the questions of step 5.
- Do not restate the knowledge base in the record. Cite the file and item.
