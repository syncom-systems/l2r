import Lean
import CrossingV1Properties

/-!
# Negative fixture for the kernel-replay gate

A declaration added with `debug.skipKernelTC` never reaches the kernel. Here it
"proves" `False`, and with it any system property. It compiles, and it depends
only on the standard axioms, so the axiom audit alone would accept a theorem
built on it.
The lane requires `leanchecker` to reject this module. Never imported by a proof.
-/

open Lean Elab Command

set_option debug.skipKernelTC true in
run_cmd liftCoreM do
  addDecl (.thmDecl { name := `KernelForgery.forged, levelParams := [],
                      type := mkConst ``False, value := mkConst ``True.intro })

theorem KernelForgery.everything_coherent (k : L2R.Async1.Config CrossingV1Properties.N) :
    CrossingV1Properties.Coherent k :=
  KernelForgery.forged.elim

run_cmd do
  let axioms ← liftCoreM (collectAxioms `KernelForgery.everything_coherent)
  logInfo m!"FORGERY AXIOMS {axioms.toList}"
