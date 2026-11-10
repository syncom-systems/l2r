# CodeScene findings: cause, honest fix, gaming move

Ordered by how often each finding appeared in past CodeScene sessions. "Gaming" moves improve the score while making the design worse; `arch-verify` checks for them.

| Finding | Usual cause | Honest fix | Gaming move |
|---|---|---|---|
| **Low Cohesion** | Several responsibilities that change for different reasons share one module or class. | Group the functions by the data or concept they touch, and give each group its own module named after that concept. | Splitting by size or alphabetically. The new files stay coupled and always change together. |
| **Complex Method** / **Overall Code Complexity** | One concept (status, kind, policy) is re-decided inline with branches. | Find what the branches discriminate on. Variants of one concept get polymorphism or a strategy owned by that concept. Validation gets guard clauses or a validator. Real domain complexity gets an explicit, named decision table with tests. | Anonymous dict dispatch; one helper per branch. |
| **Large Method** | Steps at mixed levels of abstraction. | Extract the steps that have a domain name, so the method reads as the procedure. Extract only what you can name without "and". | `_part1` / `_part2`; extractions that take 6 parameters and return a tuple. |
| **Excess Number of Function Arguments** | A missing concept whose fields travel together (period start+end, amount+currency), or a function doing several things. | If the arguments always travel together and share an invariant, make a value object named after the concept. If not, split the function. | `**kwargs`; an `Options` / `Context` / `Params` bag of unrelated fields; defaults that hide required inputs. |
| **Code Duplication** | One rule written in several places, or several similar-looking but different rules. | Decide first whether it is the same rule, meaning it changes together. If so, give it one owner. If the code only looks alike, keep it and say so. | A shared `utils` function with flags that select each caller's variant. |
| **Brain Method** / **Brain Class** | The centre of a missing model: long, complex and nested, touching many fields. | A design task, not a mechanical one. Identify the concepts with `codebase-design` and move the behaviour onto them. Usually Module or Architectural level. | Slicing it into private helpers that still all read and write the same fields. |
| **String Heavy Function Arguments** / **Primitive Obsession** | IDs, codes, money and enumerations passed as `str`/`int`/`float`. | Typed IDs, value objects, enums. This often fixes real argument-order bugs too. | Aliases or wrappers that enforce no invariant. |
| **Bumpy Road Ahead** | Several nested blocks in sequence, i.e. several responsibilities. | Each bump is a named step or belongs to the object whose data it inspects. | Extracting the bumps under generic names. |
| **Complex Conditional** | A boolean expression that encodes a business rule. | Give the rule a name: a predicate on the owning object, or a policy. | `cond1`, `cond2` locals. |
| **Deep, Nested Complexity** | Preconditions and iteration interleaved. | Guard clauses for the preconditions, then a named query or collection operation. | Wrapping the inner block in a closure. |
| **Lines of Code in a Single File** / **Number of Functions in a Single Module** | Same as Low Cohesion. | Split by concept, not by size. | Splitting at a line count. |
| **Large / Duplicated Assertion Blocks** (tests) | A test that checks many fields or repeats setup. | Parametrized or table-driven cases, assertions on a whole expected value, shared fixtures. One property per test. | An `assert_everything(obj)` helper that hides which property a test is about. |
| **Constructor Over-Injection** | A class with too many roles. | Split the roles. A named facade over a real subsystem is fine. | Injecting the container or a service locator. |

## Observed in trials

Confirmed in past CodeScene sessions in other repositories. When a new move can be detected mechanically, add it to `arch-verify/scripts/arch_check.py`; the ones marked ✓ already are.

- **A service file, 7.35 → 9.09 in minutes.** Three moves:
  - a frozen data class carrying two lambdas ✓;
  - a "generic, callable-driven" resolver that expressed no domain concept ✓;
  - an interface invented to get past a hook ✓ (single-implementation check).

  It broke 19 tests. The remedy that worked was redesigning around the domain concept.
- **Code hidden from the analyser.** Macro-generated shims ✓ and facades that only re-export ✓ ("CodeScene can't score it", reported as unscorable). A 10.0 on a file the tool cannot read proves nothing.
- **Splits that force exposure.** Private helpers made public ✓ and fields widened to `pub(super)` ✓ so that split files could reach each other. The split cut through one module.
- **Moving tests to shift a ratio.** Moving tests to lower a String-Heavy ratio, rather than introducing the missing type.
- **Lowering the repository's own debt baselines** alongside the refactor ✓. That edits the measurement.
- **"Reviewed exception" for a long sequential function.** The user overruled it and the function was decomposed. A complexity finding is not closed by an argument.
- **Excess Arguments.** Parameter structs were accepted when they named a real concept. User: "If you have five or ten parameters, that's sick … CodeScene numbers mean nothing if you write sloppy code to get high numbers."
