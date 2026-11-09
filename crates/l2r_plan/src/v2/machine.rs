//! Profile `transition-core/v1`: a pure state machine whose types may carry data.
//!
//! Generates three files per machine component:
//!
//! * `mod.rs` (generated): the contract. `State`, `Input` and `Output` are
//!   aliases of the mirrors of the Lean types in `crate::types`, and
//!   `init`/`step` are wrappers whose signatures the implementation must satisfy;
//! * `contract_tests.rs` (generated): the transition is defined (does not
//!   panic, in particular on integer overflow) on representative states and
//!   inputs, including the boundary values of every field;
//! * `imp.rs` (implementation): stubs for the implementer, created once.

use l2r_model::{Behavior, Component, profiles::TRANSITION_CORE_V1};

use crate::plan::{Artifact, Ownership};
use crate::text;
use crate::v2::Gen;
use crate::{PlanError, planned_path};

pub fn artifacts(generator: &Gen<'_>, component: &Component) -> Result<Vec<Artifact>, PlanError> {
    let behavior = component
        .behavior
        .as_ref()
        .ok_or_else(|| PlanError(format!("machine `{}` has no behavior", component.id)))?;
    let dir = &generator.dirs[&component.id];
    let mut lean = vec![
        behavior.owned_state.clone(),
        behavior.input.clone(),
        behavior.output.clone(),
        behavior.init.clone(),
        behavior.step.clone(),
    ];
    lean.extend(behavior.invariants.iter().cloned());
    lean.sort();
    lean.dedup();
    let artifact = |name: &str, ownership, purpose: &str, content| -> Result<Artifact, PlanError> {
        Ok(Artifact {
            path: planned_path(format!("{dir}/{name}"))?,
            ownership,
            profile: TRANSITION_CORE_V1.into(),
            purpose: purpose.into(),
            components: vec![component.id.clone()],
            lean: lean.clone(),
            content,
        })
    };
    Ok(vec![
        artifact(
            "mod.rs",
            Ownership::Generated,
            "Machine contract: types and transition signature",
            contract(generator, component, behavior)?,
        )?,
        artifact(
            "contract_tests.rs",
            Ownership::Generated,
            "Generated contract tests",
            contract_tests(behavior),
        )?,
        artifact(
            "imp.rs",
            Ownership::Implementation,
            "Transition implementation, owned by the implementer",
            implementation(component, behavior),
        )?,
    ])
}

fn contract(
    generator: &Gen<'_>,
    component: &Component,
    behavior: &Behavior,
) -> Result<String, PlanError> {
    let mut out = text::generated_marker(TRANSITION_CORE_V1);
    out.push('\n');
    out.push_str(&text::inner_doc(&module_doc(component, behavior)));
    out.push_str("\nmod imp;\n\n#[cfg(test)]\nmod contract_tests;\n\n");
    for (alias, what, lean) in [
        ("State", "Owned state", &behavior.owned_state),
        ("Input", "Inputs accepted", &behavior.input),
        ("Output", "Outputs emitted", &behavior.output),
    ] {
        out.push_str(&format!(
            "/// {what}; mirrors `{lean}`.\npub type {alias} = {};\n\n",
            generator.type_path(lean)?
        ));
    }
    out.push_str(&format!(
        "/// The initial state; implements `{init}`.\n\
         pub fn init() -> State {{\n    imp::init()\n}}\n\n\
         /// The transition function; implements `{step}`.\n\
         pub fn step(state: State, input: Input) -> (State, Option<Output>) {{\n    imp::step(state, input)\n}}\n",
        init = behavior.init,
        step = behavior.step
    ));
    Ok(out)
}

fn module_doc(component: &Component, behavior: &Behavior) -> String {
    let mut doc = format!(
        "{} (`{}`).\n\nResponsibility: {}\n",
        component.name, component.id, component.responsibility
    );
    if let Some(notes) = &component.notes {
        doc.push_str(&format!("\nArchitect's notes: {notes}\n"));
    }
    doc.push_str(&format!(
        "\n# Lean references\n\n\
         - owned state: `{}`\n- input: `{}`\n- output: `{}`\n- initial state: `{}`\n- transition: `{}`\n",
        behavior.owned_state, behavior.input, behavior.output, behavior.init, behavior.step
    ));
    doc.push_str(
        "\n# Obligations\n\n\
         `imp.rs` must agree with the Lean transition on every state and input,\n\
         including integer arithmetic: Lean's fixed-width integers wrap, and Rust's\n\
         `+` does not. The theorems below are proved about the Lean specification;\n\
         they say nothing about this implementation until correspondence is\n\
         established. Passing Rust tests does not establish it.\n",
    );
    if behavior.invariants.is_empty() {
        doc.push_str("\n(No invariants are referenced.)\n");
    } else {
        doc.push('\n');
        for invariant in &behavior.invariants {
            doc.push_str(&format!("- `{invariant}`\n"));
        }
    }
    doc
}

fn contract_tests(behavior: &Behavior) -> String {
    let mut out = text::generated_marker(TRANSITION_CORE_V1);
    out.push('\n');
    out.push_str(&text::inner_doc(&format!(
        "Contract tests: properties every implementation of `{}` must have.\n\
         \n\
         They check that the transition is defined (does not panic, in particular on\n\
         integer overflow) on representative states and inputs: every constructor,\n\
         with the boundary values of each field. They are not exhaustive and do not\n\
         check agreement with the Lean specification.",
        behavior.step
    )));
    out.push_str(
        "\nuse super::{Input, State, init, step};\n\n\
         #[test]\n\
         fn init_is_defined() {\n    let _ = init();\n}\n\n\
         #[test]\n\
         fn step_is_defined_on_representative_values() {\n    \
             for input in Input::samples() {\n        \
                 let _ = step(init(), input);\n        \
                 for state in State::samples() {\n            \
                     let _ = step(state, input);\n        \
                 }\n    \
             }\n}\n",
    );
    out
}

fn implementation(component: &Component, behavior: &Behavior) -> String {
    let mut doc = format!(
        "Implementation of {} (`{}`).\n\
         \n\
         This file belongs to the implementer. L2R created it once and will never\n\
         overwrite it. Stay within the contract in `mod.rs`: do not add state, I/O or\n\
         public items, and report contradictions with the Lean specification instead\n\
         of working around them.\n",
        component.name, component.id
    );
    if let Some(budget) = component.implementation_budget_lines {
        doc.push_str(&format!(
            "\nSize budget: at most {budget} lines (checked by `l2r check`).\n"
        ));
    }
    format!(
        "{}\nuse super::{{Input, Output, State}};\n\n\
         /// Implements `{init}`.\n\
         pub(super) fn init() -> State {{\n    todo!(\"implement {init}\")\n}}\n\n\
         /// Implements `{step}`.\n\
         pub(super) fn step(state: State, input: Input) -> (State, Option<Output>) {{\n    \
             let _ = (state, input);\n    \
             todo!(\"implement {step}\")\n}}\n",
        text::inner_doc(&doc),
        init = behavior.init,
        step = behavior.step
    )
}
