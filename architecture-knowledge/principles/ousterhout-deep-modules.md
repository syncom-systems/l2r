---
id: ousterhout-deep-modules
title: Modules should be deep
source: J. Ousterhout, CS 190 "Software Design Studio" lecture notes, "Modular Design", Stanford University, Winter 2019. Copy used https://web.stanford.edu/~ouster/cgi-bin/cs190-winter19/lecture.php?topic=modularDesign. The notes give as reading "Chapters 4-7, 14" of his book *A Philosophy of Software Design* (Yaknyam Press, 2018).
checked: claims tagged [notes] against the lecture notes; claims tagged [book] are not checked, because no copy of the book was available
status: provisional
related: parnas-1972-information-hiding, liskov-zilles-1974-abstract-data-types
---

Quote only the [notes] claims as Ousterhout's words. Treat [book] claims as unchecked until someone verifies them against the book.

## Source

- **Interface and implementation** [notes]. The interface is "anything about the module that must be known to other modules", with formal aspects ("method signatures, public variables, etc.") and informal aspects ("overall behavior, side effects, constraints on usage, etc."), which "can only be described with comments". "Goal: interface should be much simpler than the implementation".
- **Deep and shallow** [notes]. "Deep class: small interface, lots of functionality". A shallow class has a "Complex interface and/or not much functionality"; "Invoking a method isn't much easier than just typing in the code of the method." Examples: Unix system calls for file I/O (deep); a linked list (shallow). "Every class and method introduces complexity with its interface".
- **Size is not the criterion** [notes]. "Many courses teach students that 'classes should be small': results in shallow classes." "Classitis: small classes taken to the extreme". "Size doesn't really matter that much … The most important thing is depth: the power of the abstraction". "It's more important for a class to have a simple interface than a simple implementation".
- **Information hiding and leakage** [notes]. Information hiding, credited to Parnas 1972, is "the single most important idea in software design". "Information leakage: opposite of information hiding … Anything in the interface is leaked"; "Back-door leakage: not visible in the interface". "Temporal decomposition: one of the most common causes of information leakage": "Code structure reflects the order in which operations execute". "When you see information leakage, look for a way to bring all the information together in one place".
- **Somewhat general-purpose** [notes]. "make classes somewhat generic": "Overall capabilities reflect current needs", but "Design an interface that is generic enough to be used for other purposes besides today's needs". "Result: simpler and deeper interface than special-purpose approach".
- **New layer, new abstraction** [notes]. "Each layer's abstraction should be different from the layer above it and the layer below it. Red flag: pass-through methods". Java I/O is given as a bad example of interface design; device-independent Unix I/O (open, read, write, seek, close, plus one call for device-specific operations) as a good one.
- **Pull complexity down** [notes]. "Let a few module developers suffer, rather than thousands of users"; "Simple APIs are more important than a simple implementation". "Handle error conditions rather than throwing exceptions". "Minimize 'voodoo constants' (configuration parameters)", because "If you don't know the right value, how will a user or administrator ever figure it out?"
- **Long methods** [notes]. "Shorter is generally better, but only decompose if it can be done cleanly (are there dependencies between the parts?)."
- **Design twice** [notes]. "Make 2 designs and compare".
- [book] The chapter structure (chapter 4 "Modules Should Be Deep", and the chapters on leakage, layers and pulling complexity down), the phrase "define errors out of existence", and the composed-stream detail of the Java I/O example come from the book and summaries of it. They are not checked.

## Interpretation

1. **Judge a module by what a caller must learn against what it gets.** The interface includes the informal part: order of calls, preconditions, side effects, error behaviour. Size is not the measure.
2. **A layer that adds no abstraction is cost without benefit.** Forwarding methods and wrappers with the same shape as what they wrap widen the system without hiding anything.
3. **Complexity a module could absorb, it should absorb.** Configuration the module could decide, and error cases it could handle, are complexity pushed onto every caller.
4. **Splitting is justified by independence, not by length.** Two parts that share knowledge belong together; dividing them creates leakage.

## Application

**Questions for a design**
- What must a caller know to use this module correctly, including order, preconditions and errors? Is that much less than what the module does?
- What would every caller do if this module did not exist? If "almost nothing", it is shallow.
- Does this layer offer a different abstraction from the one below it?
- What does this module push to its callers (parameters, ordering rules, error cases) that it could settle itself?
- Is this split along independent parts, or along the order in which things happen?

**How a breach shows in code**
- Pass-through methods; wrappers with the same signature as what they wrap.
- An interface about as long as the body behind it.
- Callers that must call several operations in a fixed order to do one thing.
- Configuration parameters the module could have decided.
- The same knowledge needed on both sides of an interface (leakage), often from a split by processing order.

**Not a breach**
- A small module with a tiny interface that hides a real decision: deep relative to its interface.
- A thin adapter at a deliberate boundary, such as one per external system, whose job is translation.

## Tensions

- **Small testable units.** Splitting a deep module into many shallow ones makes each piece easy to test and the whole harder to use and change. Depth is judged at the interface callers use; the inside may be fine-grained without exposing it.
- **Overlap.** The vocabulary in `.claude/skills/codebase-design` (depth, leverage, locality) is adapted from this line of thinking. Where they differ, this file and the sources it cites win.

## Rulings

None specific to this principle.
