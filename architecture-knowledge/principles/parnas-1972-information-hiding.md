---
id: parnas-1972-information-hiding
title: Decompose by the design decisions each module hides
source: D. L. Parnas, "On the Criteria To Be Used in Decomposing Systems into Modules", Communications of the ACM 15(12):1053–1058, December 1972. Copy used https://www.cse.sc.edu/~mgv/csce590f20/ParnasCriteria.pdf
checked: every quote against the copy used
status: approved
related: parnas-1979-uses-hierarchy, liskov-zilles-1974-abstract-data-types, ousterhout-deep-modules
---

## Source

- **Purpose of modularization.** "(2) product flexibility—it should be possible to make drastic changes to one module without a need to change others; (3) comprehensibility—it should be possible to study the system one module at a time." (*Expected Benefits of Modular Programming*)
- **What a module is.** "a responsibility assignment rather than a subprogram." The modularization records "all 'system level' decisions (i.e. decisions which affect more than one module)." (*What Is Modularization?*)
- **The criterion.** "Every module in the second decomposition is characterized by its knowledge of a design decision which it hides from all others. Its interface or definition was chosen to reveal as little as possible about its inner workings." (*The Criteria*)
- **The method.** "we propose instead that one begins with a list of difficult design decisions or design decisions which are likely to change. Each module is then designed to hide such a decision from the others." (*Conclusion*)
- **Over-revealing is a design error.** "By prescribing the order for the shifts we have given more information than necessary and so unnecessarily restricted the class of systems that we can build without changing the definitions. … Our failure to do this in constructing the systems with the second decomposition must clearly be classified as a design error." (*Improvement in Circular Shift Module*)
- **Specific rules** *(paraphrase of The Criteria, items 1–5)*:
  - A data structure, its internal linkings, accessing procedures and modifying procedures are part of a single module.
  - The sequence of instructions needed to call a routine, and the routine itself, belong to the same module.
  - The formats of control blocks are hidden in a module, not used as interfaces.
  - Character codes, alphabetic orderings and similar data are hidden in a module.
  - The sequence in which certain items are processed is hidden within a single module, where practical.
- **The comparison test** *(paraphrase of Changeability)*: Parnas compares his two decompositions by listing likely changes (input format, keeping lines in core, packing characters, building an index for the shifts instead of storing them, alphabetizing once instead of as needed) and counting the modules each one touches. In the first decomposition, changing how lines are stored "would result in changes in every module"; in the second, "Knowledge of the exact way that the lines are stored is entirely hidden from all but module 1."
- **Efficiency** *(paraphrase of Efficiency and Implementation)*: the second decomposition can mean more calls between modules; the answer is a different implementation of the modules (for example, assembling them so calls become inline code), not a different decomposition.
- **Hierarchy is independent** *(paraphrase of Hierarchical Structure)*: a clean decomposition and a hierarchical structure are two desirable but independent properties.

## Interpretation

1. **The decision list comes first.** Before modules are named, write down the decisions that are difficult or likely to change: formats, representations, algorithms, orderings, device and platform details, policies, limits.
2. **One owner per decision.** Each listed decision is known by exactly one module. A decision several modules know is a system-level decision: either made deliberately at system level and recorded once, or a defect.
3. **Modules follow decisions, not time.** A module may be used at many moments of the processing; its boundary comes from what it hides, not from when it runs.
4. **Every extra guarantee in an interface is a leaked decision.** An order, a layout, a representation or a sequence that callers do not need is a design error even when nothing breaks today.

## Application

**Questions for a design**
- What are the difficult decisions here, and which are likely to change? (Before naming any module.)
- For each decision: which single module hides it, and which modules would change if it changed?
- Does this interface promise anything its callers do not need?
- Is this module defined by a decision it hides, or by a step in the processing? If a step, what does it hide?

**How a breach shows in code**
- The same format, limit, naming scheme, encoding or rule written in several modules.
- Modules named after processing steps (reader, parser, validator, executor) that share a data format between them.
- A data structure whose fields many modules read or write directly.
- An interface that exposes representation: public fields of an internal structure, raw table layouts, internal storage order.
- History in which one kind of change repeatedly touches the same several modules.

**Not a breach**
- A system-level decision made deliberately and defined once, which every module imports (a protocol constant, for example).
- A small module with a narrow interface: size is not the criterion; what it hides is.

**Example from this repository** (measured 2026-11). Mailbox capacity is decided in several modules: the 1..64 limit is checked in three validator modules, and the fallback for an unstated capacity is written in several planner modules, two of which disagree (1 in the v1 scheduler planner, 0 in the derived Lean network). No module was given the capacity decision, so each one that needed it made it again.

## Rulings

None specific to this principle.
