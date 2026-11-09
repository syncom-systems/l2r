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

namespace AeneasV1.Audit

def allowedAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

def expectedTheorems : List Name := [
  `AeneasV1.Refinement.new_refines,
  `AeneasV1.Refinement.step_once_refines,
  `AeneasV1.Refinement.send_button_refines,
  `AeneasV1.Refinement.send_timer_refines,
  `AeneasV1.Refinement.rustRun_inv,
  `AeneasV1.Refinement.rustRun_simulates,
  `AeneasV1.Refinement.rustRun_bounded
]

/-- The abstraction functions, the invariant and the model embedding are part of every statement's
meaning: a permuted mapping would keep the statement text while changing what it says. Their
definitions are therefore pinned as well. -/
def abstractionFunctions : List Name := [
  `AeneasV1.Refinement.latchState,
  `AeneasV1.Refinement.latchInput,
  `AeneasV1.Refinement.latchOutput,
  `AeneasV1.Refinement.signalState,
  `AeneasV1.Refinement.signalInput,
  `AeneasV1.Refinement.signalOutput,
  `AeneasV1.Refinement.RepL,
  `AeneasV1.Refinement.RepS,
  `AeneasV1.Refinement.absL,
  `AeneasV1.Refinement.absS,
  `AeneasV1.Refinement.absV,
  `AeneasV1.Refinement.Inv,
  `AeneasV1.Refinement.abs,
  `AeneasV1.Refinement.buttonMsg,
  `AeneasV1.Refinement.timerMsg,
  `AeneasV1.mkCfg,
  `AeneasV1.toSched
]

/-- Inductive predicates used in statements: their constructors are pinned. -/
def inductives : List Name := [`AeneasV1.Refinement.RustRun]

/-- One line, whitespace-normalised, so that the textual comparison pins the whole statement. -/
def flat (s : String) : String :=
  " ".intercalate (((s.replace "\n" " ").splitOn " ").filter (· != ""))

def printInductive (decl : Lean.Name) : CommandElabM Unit := do
  let some (.inductInfo info) := (← getEnv).find? decl
    | throwError "AUDIT FAILED: {decl} is missing or not an inductive"
  for ctor in info.ctors do
    let some ci := (← getEnv).find? ctor | throwError "missing constructor {ctor}"
    let ty ← liftTermElabM do
      withOptions (fun o => o.setBool `pp.fullNames true) do
        return flat ((← ppExpr ci.type).pretty 100000)
    logInfo m!"CONSTRUCTOR {ctor} : {ty}"

def printDefinition (decl : Lean.Name) : CommandElabM Unit := do
  let some (.defnInfo info) := (← getEnv).find? decl
    | throwError "AUDIT FAILED: abstraction function {decl} is missing or not a definition"
  let lines ← liftTermElabM do
    withOptions (fun o => o.setBool `pp.fullNames true) do
      match ← getEqnsFor? decl with
      | some eqns =>
        eqns.toList.mapM fun eqn => do
          let some ci := (← getEnv).find? eqn | throwError "missing equation {eqn}"
          return flat ((← ppExpr ci.type).pretty 100000)
      | none =>
        -- A definition without pattern matching: pin its type and its body.
        return [s!"{flat ((← ppExpr info.type).pretty 100000)} := {flat ((← ppExpr info.value).pretty 100000)}"]
  for l in lines do
    logInfo m!"DEFINITION {decl} : {l}"

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
      return flat ((← ppExpr info.type).pretty 100000)
  logInfo m!"STATEMENT {decl} : {statement}"

run_cmd do
  for decl in abstractionFunctions do
    printDefinition decl
  for decl in inductives do
    printInductive decl
  for decl in expectedTheorems do
    audit decl
  logInfo m!"AUDIT PASSED: {expectedTheorems.length} theorems"

end AeneasV1.Audit
