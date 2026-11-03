import Lean
open Lean

/-- `lean --run DependencyClosure.lean <Module>...`: prints `<module> <TAB> <olean path>` for every
module in the transitive import closure of the given modules (the modules themselves included),
one per line, sorted. The scheduled dependency recheck replays exactly these. -/
def main (args : List String) : IO Unit := do
  initSearchPath (← findSysroot)
  let env ← importModules (args.toArray.map fun a => { module := a.toName }) {}
  let mut lines : Array String := #[]
  for m in env.header.moduleNames do
    lines := lines.push s!"{m}\t{← findOLean m}"
  for l in lines.qsort (· < ·) do IO.println l
