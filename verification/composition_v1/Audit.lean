import Lean
import CrossingV1Properties
import CrossingV1LivenessWitness

/-!
# Composition proof audit (`composite/v1`)

Compiling the proofs is not evidence by itself. This audit:

* checks that every expected theorem exists, is a theorem, and depends on no
  axiom outside `propext`, `Classical.choice` and `Quot.sound` (rejecting
  `sorryAx`, `Lean.ofReduceBool` from `native_decide`, and new axioms);
* prints each theorem's exact statement (`STATEMENT`);
* prints the definitions that give those statements their meaning
  (`DEFINITION`, `CONSTRUCTOR`): the semantics of permitted behaviour including
  capacity and the room condition, the scheduler model, the environment, the
  finite abstraction and the predicates. A weakened semantics (for example, a
  delivery that ignores capacity) would otherwise leave every statement's text
  unchanged.

The lane compares the printed lines with the reviewed `expected-statements.txt`.
-/

open Lean Elab Command Meta

namespace CompositionAuditV1

def allowedAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

def expectedTheorems : List Name := [
  -- generic semantics and the composite/v1 scheduler model
  `L2R.Async1.fits_iff,
  `L2R.Async1.deliver_some,
  `L2R.Async1.deliver_none_iff,
  `L2R.Async1.deliver_bounded,
  `L2R.Async1.step_bounded,
  `L2R.Async1.Reachable.bounded,
  `L2R.Async1.reachable_invariant,
  `L2R.Async1.star_invariant,
  `L2R.Async1.fair_termination,
  `L2R.Async1.RoundRobin.stepOnce_sound,
  `L2R.Async1.RoundRobin.send_sound,
  `L2R.Async1.RoundRobin.run_sound,
  `L2R.Async1.RoundRobin.run_invariant,
  `L2R.Async1.RoundRobin.run_bounded,
  `L2R.Async1.RoundRobin.send_rejected_iff,
  `L2R.Async1.RoundRobin.mem_order,
  `L2R.Async1.RoundRobin.pick_isSome_iff,
  `L2R.Async1.RoundRobin.stepOnce_delivered_iff,
  `L2R.Async1.RoundRobin.stepOnce_settled_iff,
  `L2R.Async1.RoundRobin.stepOnce_blocked,
  -- the bounded, repaired crossing
  `CrossingV1Properties.step_sim,
  `CrossingV1Properties.reach_has_initial,
  `CrossingV1Properties.reach_closed,
  `CrossingV1Properties.view_reachable,
  `CrossingV1Properties.reach_coherent,
  `CrossingV1Properties.reach_bounded,
  `CrossingV1Properties.reach_live,
  `CrossingV1Properties.coherent_of_reachable,
  `CrossingV1Properties.at_most_one_request,
  `CrossingV1Properties.acknowledgement_only_to_pending,
  `CrossingV1Properties.mailboxes_bounded,
  `CrossingV1Properties.no_conflicting_release,
  `CrossingV1Properties.no_deadlock,
  `CrossingV1Properties.succs_realized,
  `CrossingV1Properties.dist_certificate,
  `CrossingV1Properties.no_stranding,
  `CrossingV1Properties.scheduler_coherent,
  `CrossingV1Properties.scheduler_bounded,
  `CrossingV1Properties.scheduler_never_blocks,
  -- liveness under stated assumptions, and the proofs that each is satisfiable and needed
  `CrossingV1Liveness.live_cond,
  `CrossingV1Liveness.step_lab,
  `CrossingV1Liveness.enabled_real,
  `CrossingV1Liveness.pending_request_acknowledged,
  `CrossingV1LivenessWitness.ledges_realized,
  `CrossingV1LivenessWitness.hypotheses_satisfiable,
  `CrossingV1LivenessWitness.starvation_possible,
  `CrossingV1LivenessWitness.fairness_needed,
  `CrossingV1LivenessWitness.quiescent_pending,
  `CrossingV1LivenessWitness.no_dead_end
]

/-- Definitions and inductive types whose meaning the statements depend on. -/
def pinned : List Name := [
  `L2R.Async.update,
  `L2R.Async1.initial,
  `L2R.Async1.Bounded,
  `L2R.Async1.post,
  `L2R.Async1.emitted,
  `L2R.Async1.msgsFor,
  `L2R.Async1.fits,
  `L2R.Async1.deliver,
  `L2R.Async1.anyEnv,
  `L2R.Async1.inject,
  `L2R.Async1.Blocked,
  `L2R.Async1.Settled,
  `L2R.Async1.StepL,
  `L2R.Async1.Step,
  `L2R.Async1.Reachable,
  `L2R.Async1.Star,
  `L2R.Async1.WeaklyFair,
  `L2R.Async1.Recurs,
  `L2R.Async1.RoundRobin.start,
  `L2R.Async1.RoundRobin.order,
  `L2R.Async1.RoundRobin.pick,
  `L2R.Async1.RoundRobin.stepOnce,
  `L2R.Async1.RoundRobin.send,
  `L2R.Async1.RoundRobin.Run,
  `L2RDerived.crossing_v1.crossing.cap,
  `CrossingV1Properties.view,
  `CrossingV1Properties.cdeliverL,
  `CrossingV1Properties.cdeliverG,
  `CrossingV1Properties.csends,
  `CrossingV1Properties.succs,
  `CrossingV1Properties.step_all,
  `CrossingV1Properties.closure,
  `CrossingV1Properties.initialC,
  `CrossingV1Properties.reach,
  `CrossingV1Properties.tokens,
  `CrossingV1Properties.CCoherent,
  `CrossingV1Properties.Live,
  `CrossingV1Properties.layers,
  `CrossingV1Properties.dist,
  `CrossingV1Properties.Coherent,
  `L2R.Async1.Trace,
  `CrossingV1Liveness.ALab,
  `CrossingV1Liveness.cpress,
  `CrossingV1Liveness.ctick,
  `CrossingV1Liveness.ledges,
  `CrossingV1Liveness.isReq,
  `CrossingV1Liveness.isAck,
  `CrossingV1Liveness.held,
  `CrossingV1Liveness.need,
  `CrossingV1Liveness.pend,
  `CrossingV1Liveness.mu,
  `CrossingV1Liveness.hlp,
  `CrossingV1Liveness.enabledA,
  `CrossingV1Liveness.labOf,
  `CrossingV1Liveness.kids,
  `CrossingV1Liveness.LiveCond,
  `CrossingV1Liveness.alab,
  `CrossingV1Liveness.IsTimerSend,
  `CrossingV1Liveness.helper,
  `CrossingV1Liveness.rank,
  `CrossingV1Liveness.Pend,
  `CrossingV1LivenessWitness.ofView,
  `CrossingV1LivenessWitness.cycle,
  `CrossingV1LivenessWitness.cycleLabels,
  `CrossingV1LivenessWitness.stuckA,
  `CrossingV1LivenessWitness.stuckB,
  `CrossingV1LivenessWitness.starveA,
  `CrossingV1LivenessWitness.starveL,
  `CrossingV1LivenessWitness.waitA,
  `CrossingV1LivenessWitness.waitB,
  `CrossingV1LivenessWitness.waitCfg,
  `CrossingV1LivenessWitness.waitL
]

def pp (e : Expr) : CommandElabM String :=
  liftTermElabM do
    withOptions (fun o => (o.setBool `pp.fullNames true).setBool `pp.proofs true) do
      -- One line per declaration, so the whole body is pinned, not only its first line.
      return ((← ppExpr e).pretty 100000).replace "\n" " "

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

end CompositionAuditV1
