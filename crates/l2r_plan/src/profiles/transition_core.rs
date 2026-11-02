//! Profile `transition-core/v0`: a pure state machine.
//!
//! Generates three files per machine component:
//!
//! * `mod.rs` (generated): the contract — enums mirroring the Lean types, and
//!   `init`/`step` wrappers whose signatures the implementation must satisfy;
//! * `contract_tests.rs` (generated): totality over every state and input;
//! * `imp.rs` (implementation): stubs for the implementer, created once.

use l2r_model::{Behavior, Component, profiles::TRANSITION_CORE_V0};
use l2r_validate::{ResolvedEnum, ResolvedMachine};

use crate::plan::{Artifact, Ownership};
use crate::text;

/// Plans the files for one machine component in directory `dir`.
pub fn artifacts(component: &Component, machine: &ResolvedMachine, dir: &str) -> Vec<Artifact> {
    let behavior = component
        .behavior
        .as_ref()
        .expect("validated machine has behavior");
    let lean = lean_refs(behavior, machine);
    let artifact = |name: &str, ownership, purpose: &str, content| Artifact {
        path: format!("{dir}/{name}"),
        ownership,
        profile: TRANSITION_CORE_V0.into(),
        purpose: purpose.into(),
        components: vec![component.id.clone()],
        lean: lean.clone(),
        content,
    };
    vec![
        artifact(
            "mod.rs",
            Ownership::Generated,
            "Machine contract: types and transition signature",
            contract(component, behavior, machine),
        ),
        artifact(
            "contract_tests.rs",
            Ownership::Generated,
            "Generated contract tests",
            contract_tests(behavior),
        ),
        artifact(
            "imp.rs",
            Ownership::Implementation,
            "Transition implementation, owned by the implementer",
            implementation(component, behavior),
        ),
    ]
}

fn lean_refs(behavior: &Behavior, machine: &ResolvedMachine) -> Vec<String> {
    let mut refs = vec![
        behavior.owned_state.clone(),
        behavior.input.clone(),
        behavior.output.clone(),
        behavior.init.clone(),
        behavior.step.clone(),
    ];
    refs.extend(behavior.invariants.iter().cloned());
    for e in [&machine.state, &machine.input, &machine.output] {
        refs.extend(e.variants.iter().map(|v| v.lean_name.clone()));
    }
    refs.sort();
    refs.dedup();
    refs
}

fn contract(component: &Component, behavior: &Behavior, machine: &ResolvedMachine) -> String {
    let mut out = text::generated_marker(TRANSITION_CORE_V0);
    out.push('\n');
    out.push_str(&text::inner_doc(&module_doc(component, behavior)));
    out.push_str("\nmod imp;\n\n#[cfg(test)]\nmod contract_tests;\n\n");
    out.push_str(&enum_def("State", "Owned state", &machine.state));
    out.push_str(&enum_def("Input", "Inputs accepted", &machine.input));
    out.push_str(&enum_def("Output", "Outputs emitted", &machine.output));
    out.push_str(&format!(
        "/// The initial state; implements `{init}`.\n\
         pub fn init() -> State {{\n    imp::init()\n}}\n\n\
         /// The transition function; implements `{step}`.\n\
         pub fn step(state: State, input: Input) -> (State, Option<Output>) {{\n    imp::step(state, input)\n}}\n",
        init = behavior.init,
        step = behavior.step
    ));
    out
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
         `imp.rs` must agree with the Lean transition on every state and input. The\n\
         theorems below are proved about the Lean specification; they say nothing about\n\
         this implementation until correspondence is established. Passing Rust tests\n\
         does not establish it.\n",
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

fn enum_def(name: &str, what: &str, resolved: &ResolvedEnum) -> String {
    let mut out = format!(
        "/// {what}; mirrors `{}`.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum {name} {{\n",
        resolved.lean_name
    );
    for variant in &resolved.variants {
        out.push_str(&format!(
            "    /// `{}`\n    {},\n",
            variant.lean_name, variant.rust_name
        ));
    }
    let all: Vec<String> = resolved
        .variants
        .iter()
        .map(|v| format!("{name}::{}", v.rust_name))
        .collect();
    out.push_str(&format!(
        "}}\n\nimpl {name} {{\n    /// Every value, in Lean constructor order.\n    pub const ALL: [{name}; {}] = {};\n}}\n\n",
        all.len(),
        array_literal(&all)
    ));
    out
}

/// An array literal laid out as rustfmt does: on one line up to its default
/// `array_width` of 60 characters, otherwise one element per line.
fn array_literal(items: &[String]) -> String {
    let single = format!("[{}]", items.join(", "));
    if single.len() <= 60 {
        return single;
    }
    let mut out = String::from("[\n");
    for item in items {
        out.push_str(&format!("        {item},\n"));
    }
    out.push_str("    ]");
    out
}

fn contract_tests(behavior: &Behavior) -> String {
    let mut out = text::generated_marker(TRANSITION_CORE_V0);
    out.push('\n');
    out.push_str(&text::inner_doc(&format!(
        "Contract tests: properties every implementation of `{}` must have.\n\
         \n\
         They check totality over every state and input. They do not check agreement\n\
         with the Lean specification.",
        behavior.step
    )));
    out.push_str(
        "\nuse super::{Input, State, init, step};\n\n\
         #[test]\n\
         fn init_is_defined() {\n    let _ = init();\n}\n\n\
         #[test]\n\
         fn step_is_total() {\n    for state in State::ALL {\n        for input in Input::ALL {\n            let _ = step(state, input);\n        }\n    }\n}\n",
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
