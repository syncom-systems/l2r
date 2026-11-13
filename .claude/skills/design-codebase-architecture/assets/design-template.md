# Design: <system name>

Architect: <name>. Knowledge base: `architecture-knowledge/` (rulings first).

Ids: G given facts, D decisions, M modules, U uses edges (cited by the diagram's relations), I interfaces, Q questions. Each row or interface section has one mark:
- **GIVEN (source)**: a fact from the brief, an ADR or the code
- **DECIDED (architect, date)**
- **PROPOSED**, citing a principle (file and interpretation item), a ruling, or evidence
- **OPEN**

## Given

Facts and binding constraints the design must respect: the brief, accepted ADRs, documented rules, existing interfaces. Quote each with its source.

| Id | Fact | Source | Mark |
|---|---|---|---|
| G1 | | | GIVEN |

## Decisions to hide

| Id | Decision | Why it is difficult or may change | Hidden in (a module, or system-level) | Mark |
|---|---|---|---|---|
| D1 | | | M1 | PROPOSED (parnas-1972-information-hiding, Interpretation 1) |

## Modules

| Id | Module | Hides | Owns these nouns | Mark |
|---|---|---|---|---|
| M1 | | D1 | | PROPOSED |

**Change scenarios**

| Change | Modules touched | Mark |
|---|---|---|
| | M1 | PROPOSED |

## Uses relation

| Id | A uses B (and any reliance without a call, or on order) | a) A simpler | b) B not harder | c) subset with B, not A | d) no subset with A, not B | Mark |
|---|---|---|---|---|---|---|
| U1 | M2 → M1 | | | | | PROPOSED |

Levels, and what each level with everything below it can do:

- Level 0: …

## Interfaces

### I1: M1

```rust
// public types and signatures
```

- **Guarantees carried by types:** …
- **Obligations the signatures do not show:** call order, preconditions, side effects, errors.
- **Depth:** what a caller must know against what the module does; pass-through parameters or layers.
- **Alternatives** (for interfaces used by more than one module, or hiding a volatile decision): A … / B … — what the caller must know, calls per intent, what stays hidden.

Mark: PROPOSED

## Questions for the architect

In the order they must be decided.

| Id | Question | Alternatives and consequences | Recommendation, and why | Unblocks |
|---|---|---|---|---|
| Q1 | | | | |
