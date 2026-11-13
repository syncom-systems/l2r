---
id: parnas-1979-uses-hierarchy
title: Design the "uses" relation deliberately, and keep it loop-free
source: D. L. Parnas, "Designing Software for Ease of Extension and Contraction", IEEE Transactions on Software Engineering SE-5(2):128–138, March 1979. Copy used https://www.dre.vanderbilt.edu/~schmidt/PDF/family.pdf
checked: every quote against the copy used (two passages garbled by column extraction were confirmed by reading the page)
status: approved
related: parnas-1972-information-hiding, liskov-zilles-1974-abstract-data-types
---

## Source

- **Definition.** "A uses B if correct execution of B may be necessary for A to complete the task described in its specification." That is, A "requires the presence of a correct version of" B. (*Designing the "Uses" Structure*)
- **Uses is not invokes** *(paraphrase, same section)*: an invocation is not a use when A's specification only requires that A make the call. A may use B without calling it: programs use an interrupt handler they never call.
- **The criteria.** Allow A to use B "when all of the following conditions hold:
  - a) A is essentially simpler because it uses B;
  - b) B is not substantially more complex because it is not allowed to use A;
  - c) there is a useful subset containing B and not A;
  - d) there is no conceivably useful subset containing A but not B."
- **Levels.** If the relation is loop-free, "level 0 is the set of all programs that use no other program; level i (i ≥ 1) is the set of all programs that use at least one program on level i−1 and no program at a level higher than i−1." "each level offers a testable and usable subset of the system."
- **A milestone.** "The design of the 'uses' hierarchy should be one of the major milestones in a design effort." It goes "in parallel with the decisions about" how the system divides into callable programs, "because they influence each other."
- **Sandwiching.** When A would benefit from using B and B from using A, "we may split B into two programs: B1 and B2. We then allow A to use B2 and B1 to use A."
- **Modules are not levels.** The unit of change is the module, "not a single callable subprogram"; one module's programs may sit on different levels. "In most cases the phrase 'levels of abstraction' is an abuse of language." (*On the Distinction Between Modules, Subprograms, and Levels*)
- **Subsets are requirements.** "the identification of useable subsets" is "part of the preliminaries to software design. Flexibility cannot be an afterthought." (*Summation*)
- **Example** *(paraphrase)*: in T.H.E., the transput stream mechanism uses the segmenting mechanism and is simpler for it; by contrast, systems in which spooling, virtual memory and the file system "all perform their own backup store operations" are large because those parts are independent. In the operating-system family design, sandwiching split virtual memory into address translation and dynamic allocation of memory areas to segments.
- **Convenience** *(paraphrase)*: Parnas acknowledges that convenience is often the honest reason a facility sits at a given level; the criteria are what that convenience is checked against.

## Interpretation

1. **The uses relation is designed, not discovered.** It is written down during design, alongside the module decomposition.
2. **Every edge is justified by criteria a–d.** An edge that only saves typing, or exists because the code happened to make it convenient, is not justified.
3. **No loops.** A mutual dependency is resolved by splitting one side, not by accepting the cycle.
4. **Each level is a subset the design can name**: what the system minus everything above that level can still do.
5. **Reliance without a call is a use** *(our extension of "uses is not invokes")*: a component whose correct behaviour A silently relies on, such as an earlier validation step, is a "uses" edge even if A never calls it, and belongs in the design like any other.

## Application

**Questions for a design**
- For this dependency: is A essentially simpler because of B? Is there a useful system with B and without A? Would anyone want A without B?
- What are the minimal useful subsets of this system, and the minimal extensions? (Requirements, asked before the structure.)
- These two parts want to use each other: which one splits, and into what?
- What does this module rely on being correct that it never calls?

**How a breach shows in code**
- A dependency cycle between modules or crates.
- A lower-level module that imports a higher one or names its types.
- A module that is correct only because another ran first, with nothing in the design or the types recording that edge.
- Several "independent" parts that each implement the same lower service themselves.
- No usable subset: removing a part leaves what Parnas calls a need to "fill the hole" with conversion programs (*Summation, "Advantages of the Virtual Machine Approach"*).

**Not a breach**
- A call from a lower level to an upper one through a callback whose correctness the lower level does not depend on: an invocation, not a use (source).
- A module whose programs sit on different levels (source).

**Example from this repository** (read 2026-11). The planner uses the validator's acyclicity check without calling it (interpretation 5). At the crate boundary that edge is recorded in a type: `l2r_plan::plan` accepts only `l2r_validate::Validated`, which only the validator constructs. Inside the planner, the private `layout::component_dirs` takes a plain `&Model` and walks parent links with `while let`; it terminates only because of that earlier check, which its parameter type no longer carries. See [liskov-zilles-1974-abstract-data-types](liskov-zilles-1974-abstract-data-types.md) for carrying such a guarantee in a type.

## Rulings

None specific to this principle.
