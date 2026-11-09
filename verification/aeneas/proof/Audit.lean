import Refinement

/-!
# Proof audit

Compiling `Refinement.lean` is not evidence by itself: a file can compile while
proving nothing (`#exit`, `sorry`, `native_decide`, a new `axiom`, or a weakened
statement). This audit turns the claim into checks:

* every expected theorem exists and is a theorem;
* it depends on no axiom outside `propext`, `Classical.choice` and `Quot.sound`
  (this rejects `sorryAx`, `Lean.ofReduceBool` and any translation-introduced
  axiom);
* its exact statement is printed as `STATEMENT <name> : <type>`, which the test
  compares with the reviewed `expected-statements.txt`.

Any failure is an error, so this file does not compile.
-/

open Lean Elab Command Meta

namespace Crossing.Audit

def allowedAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

def expectedTheorems : List Name := [
  `Crossing.Refinement.latch_init_refines,
  `Crossing.Refinement.latch_step_refines,
  `Crossing.Refinement.signal_init_refines,
  `Crossing.Refinement.signal_step_refines,
  `Crossing.Refinement.latch_to_signal_refines,
  `Crossing.Refinement.signal_to_latch_refines,
  `Crossing.Refinement.impl_walk_entered_from_amber,
  `Crossing.Refinement.impl_request_implies_pending
]

/-- The abstraction functions are part of every statement's meaning: a permuted
mapping would keep the statement text while changing what it says. Their
definitions are therefore pinned as well. -/
def abstractionFunctions : List Name := [
  `Crossing.Refinement.latchState,
  `Crossing.Refinement.latchInput,
  `Crossing.Refinement.latchOutput,
  `Crossing.Refinement.signalState,
  `Crossing.Refinement.signalInput,
  `Crossing.Refinement.signalOutput
]

def printDefinition (decl : Lean.Name) : CommandElabM Unit := do
  let some (.defnInfo info) := (← getEnv).find? decl
    | throwError "AUDIT FAILED: abstraction function {decl} is missing or not a definition"
  let equations ← liftTermElabM do
    let some eqns ← getEqnsFor? decl | throwError "AUDIT FAILED: no equations for {decl}"
    eqns.toList.mapM fun eqn => do
      let some ci := (← getEnv).find? eqn | throwError "missing equation {eqn}"
      withOptions (fun o => o.setBool `pp.fullNames true) do
        return (← ppExpr ci.type).pretty 100000
  let _ := info
  for eqn in equations do
    logInfo m!"DEFINITION {decl} : {eqn}"

def audit (decl : Lean.Name) : CommandElabM Unit := do
  let env ← getEnv
  let some info := env.find? decl
    | throwError "AUDIT FAILED: expected theorem {decl} does not exist"
  unless info matches .thmInfo _ do
    throwError "AUDIT FAILED: {decl} is not a theorem"
  let axioms ← liftCoreM (collectAxioms decl)
  for ax in axioms do
    unless allowedAxioms.contains ax do
      throwError "AUDIT FAILED: {decl} depends on disallowed axiom {ax}"
  let statement ← liftTermElabM do
    withOptions (fun o => o.setBool `pp.fullNames true) do
      return (← ppExpr info.type).pretty 100000
  logInfo m!"STATEMENT {decl} : {statement}"

run_cmd do
  for decl in abstractionFunctions do
    printDefinition decl
  for decl in expectedTheorems do
    audit decl
  logInfo m!"AUDIT PASSED: {expectedTheorems.length} theorems"

end Crossing.Audit
