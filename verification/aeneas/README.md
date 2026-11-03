# The proof lane: Rust-to-Lean refinement of the crossing example

```sh
bazel test --config=verify //verification/...                  # what CI runs
bazel run  --config=verify //verification/aeneas:update_translation   # after changing Rust sources
```

The lane is a Bazel test (`:refinement_test`, tagged `verification`). It runs
only under `--config=verify`, because its pinned toolchain (~8 GB including
mathlib's compiled libraries) is fetched on first use. CI runs it as a separate
job (`verification` in `.github/workflows/bazel.yml`) and uploads its evidence.

## What the lane checks

| Gate | Check | Catches |
| --- | --- | --- |
| 1. Inventory | A fresh export of the Lean spec equals `examples/crossing/lean-inventory.json` | A spec edited without re-exporting; the plan would be bound to a stale inventory |
| 2. Extraction | Charon + Aeneas translate the **committed** Rust sources (both machines' `mod.rs` + `imp.rs`, and `routes.rs`), and the result is byte-identical to `proof/Extracted/CrossingCore.lean` | A stale translation: proofs about code that is no longer the code |
| 3. Proof | The spec, the translation and `proof/Refinement.lean` compile | Proofs that do not check |
| 4. Audit | `proof/Audit.lean`: every expected theorem exists, is a theorem, and depends only on `propext`, `Classical.choice` and `Quot.sound`. Its statement, and the defining equations of every abstraction function, equal the reviewed `expected-statements.txt` | `sorry`, `native_decide` (`Lean.ofReduceBool`), new axioms (including the `Hash` axiom Aeneas emits), `#exit` before a theorem, weakened statements, permuted abstraction functions |
| 4b. Kernel replay | `leanchecker` re-checks every declaration of the spec, the translation and the proof modules in the kernel | A declaration that never reached the kernel, e.g. one added with `debug.skipKernelTC`. Such a declaration compiles and uses only standard axioms, so it passes gate 4 |
| 5. Mutants | Each seeded defect in `mutants.tsv` (a real edit of the Rust source) is re-extracted, and must make **its named theorem** fail and nothing else | A proof that cannot fail; a setup error masquerading as detection |
| 6. Bypasses | Each attempt in `bypasses.tsv` to pass without a real proof (`sorry`, `native_decide`, a new `axiom`, a weakened statement, `#exit`, a permuted abstraction function, a proof that skipped the kernel) must be rejected by its named gate | An audit that has quietly stopped detecting what it claims to detect |

Compiling the translation alone (gate 3 without 4–6) is not accepted as
evidence. Gates 5 and 6 run on every CI run, so the lane continuously shows
that its own checks can fail.

## What is proved

For every input, without panicking (`Result.ok`), and under the pinned
abstraction functions:

| Theorem | Rust | Lean specification |
| --- | --- | --- |
| `latch_init_refines`, `latch_step_refines` | `request_latch::{init, step}` | `Crossing.Latch.{init, step}` |
| `signal_init_refines`, `signal_step_refines` | `signal::{init, step}` | `Crossing.Signal.{init, step}` |
| `latch_to_signal_refines`, `signal_to_latch_refines` | `routes::{latch_to_signal, signal_to_latch}` | `Crossing.Routes.*` |
| `impl_walk_entered_from_amber` | spec invariant transferred to `signal::step` | `Crossing.Signal.walk_entered_from_amber` |
| `impl_request_implies_pending` | spec invariant transferred to `request_latch::step` | `Crossing.Latch.request_implies_pending` |

## What is not proved

- **The composite scheduler** (`crossing/mod.rs`: `System`, `VecDeque`
  mailboxes, round-robin). It is excluded from the harness. Translating it needs
  Aeneas models of `VecDeque` and reasoning about `&mut self` loops; that has not
  been attempted, so it is unverified.
- **Any property of the composed system.** The machines and routes are each
  correct; nothing is proved about their asynchronous composition or about all
  interleavings (kind 4 in [`docs/verification.md`](../../docs/verification.md)).
- **Platform behaviour.** rustc's compilation of the source is trusted.

## Pins and trust

All pins are in [`bazel/verification/repo.bzl`](../../bazel/verification/repo.bzl):

- Aeneas + Charon: `nightly-2026.11.02-ab34bd2` release tarballs, by sha256.
- Lean `v4.35.0` release archives, by sha256; this is the version that Aeneas
  release's library is built with. The example spec uses the same version.
- Rust `nightly-2026-09-17` `rustc`, `rust-std` and `rustc-dev`, by sha256 from
  that day's channel manifest. This is the nightly Charon's driver links against.
- mathlib at the revision in that Aeneas release's `lake-manifest.json`. Its
  compiled libraries come from mathlib's own cache (`lake exe cache get`) and
  are **not** pinned by a hash recorded here. They are trusted artifacts. Gate
  4b re-checks this repository's own declarations with `leanchecker`, but not
  the imported Aeneas and mathlib declarations. Replaying those (`leanchecker
  --fresh`) took over 55 CPU-minutes without finishing locally. That suits a
  scheduled job, not a PR gate, and it is not done yet.

Trusted base of a passing lane: the Lean kernel; Charon and Aeneas translating
faithfully; the mathlib and Aeneas compiled libraries (their declarations are not
replayed, ours are); rustc; and the abstraction functions, which are pinned and
reviewed as part of the statements.

## Platforms

Pinned for Linux x86_64 (CI) and macOS arm64. Elsewhere the repository rule
produces an "unavailable" manifest and the test fails with that reason; it never
passes vacuously.
