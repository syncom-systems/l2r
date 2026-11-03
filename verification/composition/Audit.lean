import Lean
import CrossingProperties

/-!
# Composition proof audit

Compiling the composition proofs is not evidence by itself. This audit:

* checks that every expected theorem exists, is a theorem, and depends on no
  axiom outside `propext`, `Classical.choice` and `Quot.sound` (rejecting
  `sorryAx`, `Lean.ofReduceBool` from `native_decide`, and new axioms);
* prints each theorem's exact statement (`STATEMENT`);
* prints the definitions that give those statements their meaning
  (`DEFINITION`, `CONSTRUCTOR`): the semantics of permitted behaviour, the
  scheduler model, the environment, and the system predicates. A weakened
  semantics (for example, an environment that never sends) would otherwise
  leave every statement's text unchanged.

The lane compares the printed lines with the reviewed `expected-statements.txt`.
Any failure here is an error, so this file does not compile.
-/

open Lean Elab Command Meta

namespace CompositionAudit

def allowedAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

def expectedTheorems : List Name := [
  -- generic semantics and the composite/v0 scheduler model
  `L2R.Async.reachable_invariant,
  `L2R.Async.star_invariant,
  `L2R.Async.deliver_some,
  `L2R.Async.RoundRobin.serve_eq_deliver,
  `L2R.Async.RoundRobin.stepOnce_sound,
  `L2R.Async.RoundRobin.run_sound,
  `L2R.Async.RoundRobin.run_invariant,
  `L2R.Async.RoundRobin.mem_order,
  `L2R.Async.RoundRobin.stepOnce_none_iff,
  -- the crossing composition
  `CrossingProperties.coherent_step,
  `CrossingProperties.coherent_of_reachable,
  `CrossingProperties.at_most_one_request,
  `CrossingProperties.acknowledgement_only_to_pending,
  `CrossingProperties.no_conflicting_release,
  `CrossingProperties.scheduler_coherent,
  `CrossingProperties.stranded_step,
  `CrossingProperties.stranded_forever,
  `CrossingProperties.scheduler_strands_a_request,
  `CrossingProperties.request_can_be_stranded
]

/-- Definitions and inductive types whose meaning the statements depend on. -/
def pinned : List Name := [
  `L2R.Async.update,
  `L2R.Async.initial,
  `L2R.Async.post,
  `L2R.Async.emitted,
  `L2R.Async.deliver,
  `L2R.Async.anyInput,
  `L2R.Async.Step,
  `L2R.Async.Reachable,
  `L2R.Async.Star,
  `L2R.Async.RoundRobin.start,
  `L2R.Async.RoundRobin.order,
  `L2R.Async.RoundRobin.pushAll,
  `L2R.Async.RoundRobin.serve,
  `L2R.Async.RoundRobin.stepOnce,
  `L2R.Async.RoundRobin.send,
  `L2R.Async.RoundRobin.Run,
  `CrossingProperties.Physical,
  `CrossingProperties.inflight,
  `CrossingProperties.Coherent,
  `CrossingProperties.Stranded
]

def pp (e : Expr) : CommandElabM String :=
  liftTermElabM do
    withOptions (fun o => (o.setBool `pp.fullNames true).setBool `pp.proofs true) do
      return (← ppExpr e).pretty 100000

def printPinned (decl : Name) : CommandElabM Unit := do
  match (← getEnv).find? decl with
  | some (.defnInfo info) => logInfo m!"DEFINITION {decl} := {← pp info.value}"
  | some (.inductInfo info) =>
    for ctor in info.ctors do
      let some ci := (← getEnv).find? ctor | throwError "AUDIT FAILED: missing constructor {ctor}"
      logInfo m!"CONSTRUCTOR {ctor} : {← pp ci.type}"
  | _ => throwError "AUDIT FAILED: pinned declaration {decl} is missing or of an unexpected kind"

def audit (decl : Name) : CommandElabM Unit := do
  let some info := (← getEnv).find? decl
    | throwError "AUDIT FAILED: expected theorem {decl} does not exist"
  unless info matches .thmInfo _ do
    throwError "AUDIT FAILED: {decl} is not a theorem"
  for ax in ← liftCoreM (collectAxioms decl) do
    unless allowedAxioms.contains ax do
      throwError "AUDIT FAILED: {decl} depends on disallowed axiom {ax}"
  logInfo m!"STATEMENT {decl} : {← pp info.type}"

run_cmd do
  for decl in pinned do
    printPinned decl
  for decl in expectedTheorems do
    audit decl
  logInfo m!"AUDIT PASSED: {expectedTheorems.length} theorems"

end CompositionAudit
