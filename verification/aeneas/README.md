# Experiment: Rust-to-Lean refinement of the crossing signal

**Status:** demonstrated once, on one developer machine (macOS arm64), with the
toolchain identities recorded in [`evidence/toolchain.txt`](evidence/toolchain.txt).
This is not yet part of `bazel test //...` or CI.

## Question

Can the implementation an agent writes inside an L2R scaffold be connected to the
Lean specification by a *proof*, rather than by tests and assertion?

## What was done

1. [`signal_crate.rs`](signal_crate.rs) is a one-line crate root that includes the
   **committed** `signal` module of the example: the generated contract
   (`mod.rs`) and the implementer-owned `imp.rs`. There is no copy of the code.
2. Charon translates that crate to LLBC, and Aeneas translates the LLBC to Lean
   ([`proof/Extracted/SignalCrate.lean`](proof/Extracted/SignalCrate.lean),
   committed, with Rust source line provenance in every docstring).
3. [`proof/Refinement.lean`](proof/Refinement.lean) defines abstraction functions
   from the extracted Rust types to the specification types and proves:
   - `init_refines`: the implementation's initial state abstracts to `Crossing.Signal.init`;
   - `step_refines`: for **every** state and input, the extracted `step` succeeds
     (no panic) and its result abstracts to `Crossing.Signal.step`;
   - `impl_walk_entered_from_amber`: the spec theorem `walk_entered_from_amber`
     transferred to the implementation through `step_refines`.
4. `#print axioms` shows all three depend only on `propext`; there is no `sorryAx`.
5. **Defect detection:** `run.sh` injects a defect into a temporary copy of
   `imp.rs` (a crossing request jumps straight to `Walk`, skipping amber),
   re-extracts it and re-checks the same proof. The proof fails exactly at that
   case of `step_refines` ([`evidence/defect.log`](evidence/defect.log)). The
   script requires the failure to occur inside `step_refines`, so a setup error
   cannot pass for a detection.

Reproduce:

```sh
AENEAS_DIR=<built Aeneas checkout> RUSTUP_HOME=<rustup with Charon's nightly> \
  verification/aeneas/run.sh
```

`run.sh` also requires the committed translation to be byte-identical to a fresh
extraction, so the proof cannot silently drift from the code.

## What this shows, and what it does not

| Claim | Status |
| --- | --- |
| `signal::imp::{init, step}` agree with `Crossing.Signal.{init, step}` on all inputs | **Proved** (modulo the trusted base below) |
| `signal::step` never panics | **Proved** (the `Result` is always `ok`) |
| Spec invariant `walk_entered_from_amber` holds for the implementation | **Proved** (transferred) |
| `request_latch` implementation | Unverified (same method applies; not yet done) |
| `routes.rs` translations | Unverified |
| Generated `composite/v0` scheduler (`VecDeque`, round-robin) | Unverified; Aeneas support for `std` collections was not tried |
| Properties over all message interleavings | Not modeled. The scheduler realizes one interleaving |
| The deployed binary | Not covered: rustc and the target platform are trusted |

**Trusted base:** the Lean kernel; Charon and Aeneas translating the Rust source
faithfully (the translation is not itself verified); rustc compiling the source as
Charon read it. Aeneas's standard library models contain `sorry` (`Aeneas.Std.Slice`,
`Aeneas.Std.StringIter`) and an axiom for `Hash` on `isize`. The axiom report shows
that none of them reach these theorems, but a proof that used slices could depend on them.

## Why this route

| Tool | What it does | Fit for L2R |
| --- | --- | --- |
| **Charon + Aeneas** | Translates a *safe Rust subset* to pure Lean (also F\*, Coq, HOL4). Documented limits include some nested-loop control flow and mutable references in generic instantiations. Unsafe code and concurrency are future work. | **Chosen.** The `transition-core` profile generates exactly the shape it handles well: pure functions over enums. The proof lives in Lean, next to the specification. |
| **hax** | Rust to F\* (stable), Rocq, ProVerif, SSProve, EasyCrypt (experimental). Its recommended Lean route goes *through Aeneas*. | No independent advantage for a Lean-first project today. Revisit if F\* or ProVerif obligations become relevant. |
| **Verus** | Proves Rust against specifications written in Rust syntax, using SMT. Handles code beyond plain safe Rust. | Complementary, not competing, if its specifications state only Rust-local facts: panic freedom, bounds, frame conditions. It would compete with Lean only if it restated behavior. See [`docs/verification.md`](../../docs/verification.md). A candidate for the composite scheduler's local properties. |

## Pins (not yet reproducible from public releases alone)

The experiment ran with the Charon and Aeneas builds recorded in
`evidence/toolchain.txt`, together with Lean `v4.31.0`, which matches the Aeneas
Lean backend. Before any of this enters CI, the next increment must pin a published
Aeneas/Charon release by hash and fetch it hermetically.
