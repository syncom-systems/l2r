import Lean
import L2R.Async1Ports
import L2RDerived.tank_station
import L2RDerived.relay
import L2RDerived.starter

/-!
# Audit of the `composite/v2` scheduler model and the derived flat networks

Compiling is not evidence by itself. This audit:

* checks that every expected theorem exists, is a theorem, and depends on no
  axiom outside `propext`, `Classical.choice` and `Quot.sound`;
* prints each theorem's exact statement (`STATEMENT`);
* prints the definitions that give those statements their meaning
  (`DEFINITION`): the scheduler model, and for each example the parts of the
  derived network a hierarchy determines (which children are machines, the
  routes with their composed translations, the capacities, the input ports).

The lane compares the printed lines with the reviewed `expected-statements.txt`.
-/

open Lean Elab Command Meta

namespace CompositionAuditV2

def allowedAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

def expectedTheorems : List Name := [
  `L2R.Async1.Ports.order_machine,
  `L2R.Async1.Ports.mem_order,
  `L2R.Async1.Ports.stepOnce_delivers,
  `L2R.Async1.Ports.stepOnce_sound,
  `L2R.Async1.Ports.send_sound,
  `L2R.Async1.Ports.take_sound,
  `L2R.Async1.Ports.run_sound,
  `L2R.Async1.Ports.run_invariant,
  `L2R.Async1.Ports.run_bounded,
  `L2R.Async1.Ports.send_rejected_iff,
  `L2R.Async1.Ports.pick_isSome_iff,
  `L2R.Async1.Ports.stepOnce_delivered_iff,
  `L2R.Async1.Ports.quiet_iff,
  `L2R.Async1.Ports.stepOnce_settled_iff,
  `L2R.Async1.Ports.stepOnce_blocked,
  -- the theorems the example models name as `invariants`: proved about the
  -- Lean specifications, and nothing more
  `Tank.Filter.reports_only_changes,
  `Tank.Filter.one_sample_is_not_enough,
  `Tank.Controller.high_level_closes_valve,
  `Tank.Controller.always_reports,
  `Tank.Alarm.stays_active_until_acknowledged,
  `Tank.Alarm.count_never_decreases,
  `Relay.Source.emits_only_news,
  `Relay.Sink.urgent_is_sticky,
  `Starter.Monitor.alert_only_above_limit,
  `Starter.Monitor.reset_clears
]

/-- Definitions whose meaning the statements, and every system proof about a
derived network, depend on. -/
def pinned : List Name := [
  `L2R.Async1.outboxStep,
  `L2R.Async1.Ports.start,
  `L2R.Async1.Ports.order,
  `L2R.Async1.Ports.pick,
  `L2R.Async1.Ports.quiet,
  `L2R.Async1.Ports.stepOnce,
  `L2R.Async1.Ports.send,
  `L2R.Async1.Ports.take,
  `L2R.Async1.Ports.Run,
  `L2RDerived.tank_station.station.machines,
  `L2RDerived.tank_station.station.cap,
  `L2RDerived.tank_station.station.route,
  `L2RDerived.tank_station.station.target,
  `L2RDerived.tank_station.station.inject,
  `L2RDerived.relay.plant.machines,
  `L2RDerived.relay.plant.cap,
  `L2RDerived.relay.plant.route,
  `L2RDerived.relay.plant.target,
  `L2RDerived.relay.plant.inject,
  `L2RDerived.starter.starter.machines,
  `L2RDerived.starter.starter.cap,
  `L2RDerived.starter.starter.route,
  `L2RDerived.starter.starter.target,
  `L2RDerived.starter.starter.inject
]

def pp (e : Expr) : CommandElabM String :=
  liftTermElabM do
    withOptions (fun o => (o.setBool `pp.fullNames true).setBool `pp.proofs true) do
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

end CompositionAuditV2
