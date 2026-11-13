---
id: liskov-zilles-1974-abstract-data-types
title: A type is defined by its operations; its representation is reachable only through them
source: B. Liskov and S. Zilles, "Programming with Abstract Data Types", Proceedings of the ACM SIGPLAN Symposium on Very High Level Languages, April 1974, pp. 50–59, DOI 10.1145/800233.807045 (also published in ACM SIGPLAN Notices 9(4), DOI 10.1145/942572.807045). Copy used https://ics.uci.edu/~jajones/INF102-S18/readings/13_liskov_adt.pdf
checked: every quote against the copy used, read sentence by sentence (the text extraction interleaves the two columns)
status: draft
related: parnas-1972-information-hiding, parnas-1979-uses-hierarchy, ousterhout-deep-modules
---

## Source

- **What abstraction is for.** "What we desire from an abstraction is a mechanism which permits the expression of relevant details and the suppression of irrelevant details. In the case of programming, the use which may be made of an abstraction is relevant; the way in which the abstraction is implemented is irrelevant." (*The Meaning of Abstraction*)
- **The definition.** "An abstract data type defines a class of abstract objects which is completely characterized by the operations available on those objects." "When a programmer makes use of an abstract data object, he is concerned only with the behavior which that object exhibits but not with any details of how that behavior is achieved by means of an implementation." (*same section*)
- **The representation is closed.** Contrasting clusters with SIMULA 67 classes, whose representation "is always known to the user": "In contrast to this, the rep of a cluster is not accessible outside the cluster. Operations in the cluster provide the only way to access the contents of the rep". (*Relationship to Previous Work, SIMULA Classes*)
- **Enforced, not advised.** "Because the language is strongly typed, the user is unable to make use of any implementation details." The resulting programs "are more modular, and easier to understand, modify, maintain and prove correct." (*Controlling the Use of Information*)
- **The type's author chooses its operations.** "leaving the selection of the operations to the creator of the type, as is done with operation clusters, provides a more closely tailored abstraction." Against defining only a representation and adding operations elsewhere: "the definition, instead of being collected in one place, as it is in an operation cluster, is split into distinct parts." (*Relationship to Previous Work*)
- **A type introduced to confine knowledge.** "Token is a good example of a type created to control access to implementation details." Passing raw strings would be inefficient, and passing the grammar's table index "exposes information about the representation of the grammar". So "the new type, token, is introduced to limit the distribution of information about how the grammar is represented. Now a redefinition of the grammar cluster can affect only the token cluster". And: "If an error occurs while looking up a precedence relation (like an index out of bounds), the error can only have been caused by something in the token or grammar cluster." (*Controlling the Use of Information*)
- **Decisions can be deferred behind a type.** The representation of tokens "involves a design decision. This decision can be delayed until the cluster for tokens is defined and need not be made during the coding of Polish_gen", following Dijkstra's principle "build the program one decision at a time". (*same section*)
- **The proof splits along the type.** "The proof of a program is divided into two parts: a proof that the cluster correctly implements the type, and a proof that the program using the type is correct. Only in the former proof need details of the implementation of type objects be considered; the latter proof is based only on the abstract properties of the types, which may be expressed in terms of relations among the characterizing operations for each type." (*same section*)
- **A client knows "just enough".** Polish_gen is shielded "from any physical facts concerning its input and output"; "What it does know is just enough for its needs", and "In every case its knowledge consists of the names of the operations which provide these services." The paper also notes an obligation not visible in those names: "for scan to operate correctly, infile must provide a non-blank, non-alphanumeric character on any call on infile$get or infile$peek after the end of file has been reached." (*Using Abstract Data Types*)

## Interpretation

1. **A guarantee belongs in a type that only its owner can construct.** If a value can only be produced by the operations that establish a property (validated, normalized, bounded, acyclic), every holder of the value has the property without re-checking it, and nothing else can forge it. In Rust: private fields, constructors and operations in the owning module, no public access to the representation.
2. **Introduce a type to stop knowledge spreading**, as `token` does: when two modules would otherwise share a representation detail, give the detail a type owned by one of them.
3. **The interface is the operations plus their obligations.** The `infile` end-of-file note shows a requirement that the operation names do not carry; it is part of the interface and must be written down.
4. **Split the correctness argument at the type.** Prove or test that the type keeps its invariant once, inside it; reason about clients only from its abstract properties.

## Application

**Questions for a design**
- Which properties does each module rely on, and which type carries each one? Can a value lacking the property reach the module?
- Who may construct this type? Is that the module that establishes its invariant?
- Which representation details would cross a module boundary without a new type?
- What obligations does this interface have beyond its signatures, and where are they written?

**How a breach shows in code**
- Public fields on a type whose invariants matter, or constructors anyone may call with any values.
- A module that re-checks, or silently assumes, a property established elsewhere because the value's type does not carry it.
- Raw indices, strings or integers passed between modules where one module's internal representation gives them meaning.
- A data type defined in one place and its characteristic operations spread over others.

**Not a breach**
- A plain record whose fields have no invariant between them: there is nothing for operations to protect.

**In this repository.**
- Interpretation 1 is already applied at the planner's boundary: `l2r_validate::Validated` has private fields and is constructed only by `validate`, and `l2r_plan::plan` accepts nothing else. Inside the planner the guarantee is dropped again: `layout::component_dirs` receives a plain `&Model` and relies on acyclicity that its parameter type does not state (see [parnas-1979-uses-hierarchy](parnas-1979-uses-hierarchy.md)).
- The proof split maps onto L2R's division of authority: Lean states and proves behaviour against the abstract model, and Rust implements it. For L2R this is the intended structure, not a completed proof: the v2 scheduler and the data-carrying machines are tested against their Lean specifications, not proved to refine them (`docs/verification.md`).

## Rulings

None specific to this principle.
