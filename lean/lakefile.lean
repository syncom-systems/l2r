import Lake
open Lake DSL

/-!
L2R's Lean library as a Lake package, so that a project's own verification
package can depend on it:

    require l2r from git "https://github.com/syncom-systems/l2r.git" @ "<commit>" / "lean"

It contains the generic semantics of asynchronous composition and the models of
the generated schedulers. It has no dependencies.
-/
package l2r

@[default_target]
lean_lib L2R
