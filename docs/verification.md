# Verification in L2R: kinds of evidence and the route between them

L2R keeps one authority per fact. **Lean owns behavior.** The architecture model
owns structure. Rust is the implementation. Verification tools produce evidence
*about* these; none of them may become a second place where behavior is defined.

## Five kinds of evidence, never interchangeable

| Kind | Question answered | Tool / artifact | Status in this repository |
| --- | --- | --- | --- |
| **1. Specification proofs** | Does the Lean specification have the properties we want? | Lean theorems in the spec project (e.g. `Crossing.Signal.no_conflicting_release`) | Proved for the example spec; referenced from the model as `invariants` |
| **2. Implementation correspondence** | Does the Rust implementation compute what the Lean specification says? | Charon + Aeneas translate Rust to Lean; a refinement theorem relates the translation to the spec under explicit abstraction functions | Proved in CI for both machines (`request_latch`, `signal`) and both message routes (`verification/aeneas`). **Not** for the composite scheduler |
| **3. Rust-local properties** | Is the implementation free of panics and overflow, and are its data-structure invariants maintained, independently of behavior? | Aeneas (panic freedom follows from the `Result` being `ok`), or Verus/Kani for properties that do not restate behavior | Panic freedom proved for both machines and the routes as part of (2); nothing else yet |
| **4. Composition exploration** | Do the machines, composed asynchronously, satisfy system properties under *all* allowed interleavings? | A Lean model of the composition (or a model checker) over the space of interleavings | **Not done.** The generated scheduler realizes *one* deterministic order (see below) |
| **5. Runtime and platform evidence** | Does the deployed system behave on real hardware, with real I/O and timing? | Integration tests, hardware-in-the-loop, monitoring | Out of scope for the generator; the example has unit tests only |

A result of one kind never stands in for another. Passing Rust tests is kind 5
evidence for tested inputs. It is not correspondence (2). A proof about the spec
(1) says nothing about the code until (2) exists. A verified transition function
(2) does not verify the composed system (4).

## Scheduler order vs. the space of interleavings

`composite/v0` generates a scheduler that serves children round-robin in
identifier order, with FIFO mailboxes. That is **one** execution order, chosen to
be deterministic and testable. The abstract semantics of asynchronous
composition admits every interleaving of deliveries (subject to FIFO per
mailbox). Therefore:

- A property proved of the generated scheduler holds for that order only.
- A system property claimed for the architecture must be established over the
  space of interleavings (kind 4). Then the scheduler is shown to be one of the
  allowed behaviours (a refinement), which makes it correct by inclusion.
- Until kind 4 exists, the docs and generated comments state that the scheduler
  is unverified and one-of-many.

## Where Verus fits

Verus does not by itself create a competing behavioral authority. That happens
only if Verus specifications *restate* the behavior that Lean owns, for example
by re-encoding the transition table as `ensures` clauses. Used for kind 3
obligations, Verus complements Lean. Examples:

- no panics or arithmetic overflow in implementation code (including code Aeneas
  cannot translate, such as the `VecDeque`-based scheduler);
- bounds and data-structure invariants: mailbox capacity, index validity;
- frame conditions: a step mutates only the owning component's state.

Rule for any such tool: its specifications may mention only Rust-local facts. If
an obligation needs behavioral meaning, it must be stated in Lean and connected
through correspondence (kind 2). If that rule turns out to be too restrictive
for some property, the right fix is a generated, digest-pinned translation from
Lean, not a hand-written restatement.

## Traceability: when a proof counts

A correspondence proof is accepted only if all of the following hold. The proof
lane (`bazel test --config=verify //verification/...`) checks each of them in CI.

1. **Source identity.** The translation was produced from the exact committed
   Rust sources. It is regenerated in CI from those files, not from a copy, and
   must equal the committed translation byte for byte.
2. **Spec identity.** The proof imports the same Lean spec that produced the
   inventory that the approved plan is bound to.
3. **Tool identity.** Charon, Aeneas, Lean and the Rust nightly Charon uses are
   pinned to public releases by sha256. mathlib's compiled libraries are fetched
   at a pinned revision by mathlib's cache tool and are trusted, not hash-pinned
   here.
4. **No bypass.** Every expected theorem is present, has its expected statement,
   and depends only on the standard axioms (`propext`, `Classical.choice`,
   `Quot.sound`). `sorryAx`, `Lean.ofReduceBool` (`native_decide`) and
   translation-introduced axioms are rejected. The axiom check alone is not
   enough: a declaration added with `debug.skipKernelTC` compiles and uses only
   standard axioms. Every declaration of the spec, translation and proof is
   therefore re-checked by the kernel with `leanchecker`, and a fixture with a
   kernel-skipping proof must be rejected.
5. **Detection.** Each deliberately wrong implementation (a mutant) makes the
   proof fail inside the named refinement theorem. This shows the check can fail
   at all.
6. **Meaningful statements.** Theorem statements and the abstraction functions'
   defining equations equal a reviewed, committed expectation. A permuted
   abstraction function would otherwise keep every statement's text.

Compiling the translated Lean is not a proof check. Only the theorem statements
and the axiom report are.

## Trusted base (for kind 2)

The Lean kernel; Charon and Aeneas translating Rust faithfully (the translation
is not itself verified); rustc compiling the source as Charon read it; the
abstraction functions, which are part of the reviewed statement. Aeneas's
standard-library models contain `sorry` in some modules (e.g. slices). The axiom
check (4) ensures no accepted proof depends on them.

## Supported subset

Correspondence is practical for the code shape `transition-core/v0` generates:
pure functions over field-less enums, with `match` and no loops, heap
collections, traits or interior mutability. The `composite/v0` scheduler
(`VecDeque`, `&mut self`) is outside the subset that was demonstrated. Extending
coverage to it is planned work, with Verus (kind 3) as a candidate for its local
properties.
