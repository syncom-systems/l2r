import Lean
import L2R.Async1Ports

/-!
# Negative fixture for the kernel-replay gate

A declaration added with `debug.skipKernelTC` never reaches the kernel. Here it
"proves" `False`, and with it that a scheduler never rejects a send. It
compiles, and it depends only on the standard axioms, so the axiom audit alone
would accept a theorem built on it. The lane requires `leanchecker` to reject
this module. Never imported by a proof.
-/

open Lean Elab Command

set_option debug.skipKernelTC true in
run_cmd liftCoreM do
  addDecl (.thmDecl { name := `KernelForgery.forged, levelParams := [],
                      type := mkConst ``False, value := mkConst ``True.intro })

theorem KernelForgery.never_rejects (N : L2R.Async1.Network) (s : L2R.Async1.Ports.Sched N)
    (p : Fin N.ports) (m : N.Msg p) : L2R.Async1.Ports.send N s p m ≠ none :=
  KernelForgery.forged.elim

run_cmd do
  let axioms ← liftCoreM (collectAxioms `KernelForgery.never_rejects)
  logInfo m!"FORGERY AXIOMS {axioms.toList}"
