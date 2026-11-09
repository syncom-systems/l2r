# The `composite/v1` composition lane

```sh
bazel test --config=verify //verification/composition_v1:composition_v1_test
```

System-level proofs over every permitted interleaving of the bounded, repaired
crossing (`examples/crossing_v1`, semantics `l2r.async/v1`), and the evidence that
ties them to the architecture model and to the generated Rust. What is proved,
tested and assumed is tabulated in [`docs/composite-v1.md`](../../docs/composite-v1.md).

It is the `verification/composition` lane (see its README for the gates'
rationale) for the new semantics, plus two differences: conformance observes
**mailbox contents and send rejections**, not only states; and a fairness search
runs.

| Gate | Check | Can fail? (seeded here) |
| --- | --- | --- |
| 0. Identity | The derived composition's header names the same model, inventory and generator as the approval record, and the exact `l2r.async/v1` semantics source. `identity.txt` is written to the evidence. | model digest, inventory digest, generator, semantics version, semantics source edited after derivation |
| 1. Prove | Semantics, spec, derived network and system proofs compile. No mathlib. | — |
| 2. Audit | Every expected theorem exists and uses only `propext`/`Classical.choice`/`Quot.sound`. Statements **and the full bodies** of the semantics, scheduler model, abstraction and predicates equal `expected-statements.txt` (each definition pinned on one line). | `sorry`, `native_decide`, a new axiom, a weakened statement, a weakened definition, a weakened capacity check |
| 3. Kernel | `leanchecker --fresh` replays the proofs and everything they import, including the `decide +kernel` certificates. A kernel-skipping forgery must be rejected. | `KernelForgery.lean` |
| 4. Deviations | Seeded changes to the semantics and spec are rejected inside their named theorems: a delivery that ignores capacity, a send that ignores capacity (in the relation and in the scheduler), a scheduler serving the newest message, a signal that drops a request during walk, a timer port that injects requests. A sticky-cursor scheduler is *accepted* (still permitted) and conformance must tell it from the Rust. | `mutants.tsv` |
| 5. Bypasses | Attempts to pass without a real proof are rejected by their named gate: `sorry`, `native_decide`, a new axiom, a weakened statement, a redefined environment, a weakened timer-progress hypothesis, and `#exit` (rejected at proof checking, because later modules depend on the skipped theorems). | `bypasses.tsv` |
| 6. Conformance | **Testing, not proof.** Every sequence of 10 operations (`p` button, `t` timer, `s` step) on the generated Rust `System` and on the Lean scheduler model gives identical results (`ok`/`rej`, `D`/`S`/`B`), states and both mailboxes. | the sticky-cursor scheduler disagrees on most sequences |
| 7. Fair cycles | **Tested, not proved** (cross-check of the proved liveness theorem). Exhaustive finite-state search: a fair pending cycle exists in the original (stranded) machines and none in the repaired ones, at three capacity pairs. | the search must find the original's cycle |

What is deliberately **not** here: an Aeneas refinement of the Rust scheduler (see
`docs/composite-v1.md` for the extraction result and why the proof is not done), a
proof that the round-robin scheduler is weakly fair, and anything about real time or platforms. Liveness under fairness *is* proved, for the example at capacities (2, 2), under the hypotheses stated in `CrossingV1Liveness.lean`; the lane audits it and its satisfiability and necessity witnesses (`CrossingV1LivenessWitness.lean`).
