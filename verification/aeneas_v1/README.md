# The proof lane for the *generated* `composite/v1` scheduler

```sh
bazel test --config=verify //verification/aeneas_v1:refinement_test
bazel run  --config=verify //verification/aeneas_v1:update_translation   # after the generated Rust changes
```

Question this lane answers: **can the Rust that `composite/v1` already generates
(`examples/crossing_v1/scaffold/src/crossing/mod.rs`: `System`, array-backed mailboxes,
straight-line `step_once`, `send_*`) be proved to refine the Lean scheduler model
`L2R.Async1.RoundRobin` without changing the profile's output?** Yes. Nothing in
`crates/l2r_plan` or any generated file was changed for this lane, so the frozen plan digest
(`crates/l2r_plan/tests/composite_v1.rs`) is untouched.

## What is proved (`proof/Refinement.lean`)

Under the abstraction `abs : System -> RoundRobin.Sched N` (`N` is the derived network
`L2RDerived.crossing_v1.crossing.network`) and the invariant `Inv` (both mailboxes in canonical
array form with at most 2 messages, `next < 2`), and **without any panic** (`= ok r`):

| Theorem | Says |
| --- | --- |
| `new_refines` | `System::new` returns a system with `Inv` and `abs s = RoundRobin.start N` |
| `step_once_refines` | for every `s` with `Inv s`: `step_once` returns `Delivered` with `abs r.2` exactly the successor of `RoundRobin.stepOnce` and `Inv r.2`, or `Settled` / `Blocked` with `r.2 = s`, according to what `stepOnce` returns |
| `send_button_refines`, `send_timer_refines` | `Ok(())` with exactly the successor of `RoundRobin.send`, or `Err(Rejected)` with the system unchanged, according to what `send` returns |
| `rustRun_inv` | every system reachable through the generated API (`RustRun`: `new`, then any mix of `step_once` and sends) satisfies `Inv`, so the theorems above apply at every step |
| `rustRun_simulates`, `rustRun_bounded` | every `RustRun` system abstracts to a `RoundRobin.Run` state under the most permissive environment, so the model's run theorems (for example `run_bounded`) hold of the generated Rust |

The statements are pinned in `expected-statements.txt`, together with the definitions of every
abstraction function, the invariant, the model embedding (`toSched`, `mkCfg`) and the constructors
of `RustRun`.

## How it is proved

1. **Mailbox lemmas** (`*_push_spec`, `*_pop_spec`, `*_peek_spec`, `*_is_full_spec`, ...), against a
   canonical-form predicate `RepL`/`RepS` (`len = |l| <= 2`, first `|l|` slots `some`, rest `none`).
2. **`attempt_request_latch_spec`, `attempt_signal_spec`**, then `attempt_spec`, then
   **`step_once_spec`**, against plain functions `att0`, `att1`, `attempt`, `simOut` on the concrete
   scheduler data `V` (`proof/AeneasV1/Model.lean`).
3. **Model bridge** (`proof/AeneasV1/Model.lean`): `RoundRobin.stepOnce` / `RoundRobin.send` of the
   derived network equal `simOut` / `simSend*` on **every** well-formed `V`. Kernel
   `decide +kernel` over the complete set of 1,176 well-formed states, plus the proved coverage
   lemma `mem_allV`. No `native_decide`; the audit rejects `Lean.ofReduceBool`.

The tactic style that works with this Aeneas release: write specs as `f x ⦃ r => ... ⦄` (the
`Std.WP.spec` Hoare triple), discharge with `step` / `step with <spec> as ⟨...⟩` (the old
`progress`), and convert to `∃ r, f x = ok r ∧ ...` with `spec_ok_exists` (`cases` on the
interaction tree: `ret` / `vis` / `div`; `vis` covers `fail`). `Result` being an interaction tree
does not block anything once proofs go through the spec/`step` route instead of `rfl` / `decide`.

## Gates (see `refinement_test.sh`)

1. the committed translation (`proof/Extracted/CrossingV1Core.lean`) is byte-identical to a fresh
   Charon + Aeneas extraction of the committed generated crate;
2. everything compiles;
3. audit: expected theorems exist, use only `propext` / `Classical.choice` / `Quot.sound`, and their
   statements and the abstraction definitions equal `expected-statements.txt`;
4. `leanchecker` replays the lane's own modules through the kernel;
5. 10 seeded defects in the generated Rust (`mutants.tsv`) each make exactly their named theorem
   fail;
6. 8 bypass attempts (`bypasses.tsv`: `sorry`, `native_decide`, a new axiom, a weakened statement,
   `#exit`, a permuted abstraction, a mutated model, a forged kernel-skipping declaration) are
   rejected by their named gate.

## Honest limits

- **The model bridge is specific to this network** (2 children, capacity 2, 2 inputs each): the
  enumeration `allV` and the functions `att0`/`att1` are written for it. A different composition needs
  a new `V` and enumeration. The Rust proof is structured the same way per child, but is not generic
  in the network; for another network the lane must be re-instantiated, not reused.
- **`Inv` is a precondition, but an inductive one**: `new_refines` establishes it, and
  `step_once_refines` / `send_*_refines` preserve it (`rustRun_inv`). The theorems cover every system in
  `RustRun`, that is, every system reachable through the generated API. Rust's privacy lets code in
  descendants of `crossing` (the implementer-owned `routes.rs` and `*/imp.rs`) build a `System`
  literal that violates `Inv`; that is outside the claim (it is a code-review matter, and none of
  those files names `System`).
- **`AeneasV1.N` is not separately pinned** in `expected-statements.txt` (every statement mentions
  it). It is `L2RDerived.crossing_v1.crossing.network` by definition, and the `decide` bridge
  would fail for any other network, but the textual pin of its definition is a known gap.
- The `Hash`/`Debug`/`PartialEq` axioms Aeneas emits for std traits are in the translation but outside
  the dependency cone of everything proved (the audit would reject any dependence).
- `System::run` (a `for` loop over a range) is **not** covered; it only calls `step_once`.
  `is_settled`, `len`, `get` and the accessors are not covered either.
- Rust integer behaviour is as Aeneas models it (`Usize` with overflow as `fail`); the proof shows no
  overflow can happen from well-formed states.
- Toolchain trust is the same as `verification/aeneas`: Charon/Aeneas faithful, rustc, mathlib and
  Aeneas compiled libraries (imported declarations are not replayed).
- The lane imports `lean/L2R/Async1.lean` and the derived network read-only. If either changes, the
  audited statements or the `decide` bridge fail loudly; they are not silently re-pinned.
