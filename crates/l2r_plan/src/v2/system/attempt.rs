//! The guarded deliveries of the `System`: `attempt`, which dispatches on the
//! scheduler index, and one `attempt_<machine>` per machine.
//!
//! A delivery steps the machine, carries its output along every route, and is
//! performed only if every receiver has room for everything it would be sent.

use l2r_model::rust_names::composite_v2;
use l2r_validate::flat::{Flat, Leaf, Target};

use super::upper;
use crate::v2::Gen;

/// `attempt`: the delivery of the machine at a scheduler index.
pub(super) fn dispatch(flat: &Flat) -> String {
    let machines: String = flat
        .leaves
        .iter()
        .enumerate()
        .map(|(index, leaf)| {
            format!(
                "            {index} => self.{}(),\n",
                composite_v2::Machine(&leaf.id).attempt()
            )
        })
        .collect();
    [
        "    fn attempt(&mut self, machine: usize) -> Attempt {\n        match machine {\n",
        &machines,
        "            _ => Attempt::Idle,\n        }\n    }\n\n",
    ]
    .concat()
}

/// `attempt_<machine>` for every machine.
pub(super) fn attempts(generator: &Gen<'_>) -> String {
    generator
        .flat
        .leaves
        .iter()
        .map(|leaf| attempt(generator, leaf))
        .collect()
}

/// `attempt_<machine>`: one guarded delivery.
fn attempt(generator: &Gen<'_>, leaf: &Leaf) -> String {
    [
        step(generator, leaf),
        routed_messages(leaf),
        room_checks(leaf),
        commit(leaf),
    ]
    .concat()
}

/// The oldest message of the mailbox, if any, and the machine's step on it.
fn step(generator: &Gen<'_>, leaf: &Leaf) -> String {
    let id = &leaf.id;
    let module = generator.machine_module(id);
    let attempt = composite_v2::Machine(id).attempt();
    let inbox = composite_v2::Machine(id).inbox();
    let state = composite_v2::Machine(id).state();
    format!(
        "    fn {attempt}(&mut self) -> Attempt {{\n        let input = match self.{inbox}.peek() {{\n            Some(input) => input,\n            None => return Attempt::Idle,\n        }};\n        let (state, output) = {module}::step(self.{state}, input);\n"
    )
}

/// `m<i>`: what route `i` makes of the output.
fn routed_messages(leaf: &Leaf) -> String {
    let id = &leaf.id;
    (0..leaf.routes.len())
        .map(|i| {
            format!(
                "        let m{i} = match output {{\n            Some(output) => {id}_route_{i}(output),\n            None => None,\n        }};\n"
            )
        })
        .collect()
}

/// Room: every receiver must have room for everything this delivery sends it.
fn room_checks(leaf: &Leaf) -> String {
    receivers(leaf)
        .iter()
        .map(|(receiver, messages)| match messages[..] {
            [only] => room_for_one(receiver, only),
            _ => room_for_several(receiver, messages),
        })
        .collect()
}

/// The distinct receivers of a machine's routes, in routing order, each with
/// the routes that end there.
fn receivers(leaf: &Leaf) -> Vec<(&Target, Vec<usize>)> {
    let mut receivers: Vec<(&Target, Vec<usize>)> = Vec::new();
    for (i, route) in leaf.routes.iter().enumerate() {
        match receivers.iter_mut().find(|(to, _)| *to == &route.target) {
            Some((_, messages)) => messages.push(i),
            None => receivers.push((&route.target, vec![i])),
        }
    }
    receivers
}

/// The guard for a receiver one route ends at: blocked when it is full.
fn room_for_one(receiver: &Target, message: usize) -> String {
    format!(
        "        if m{message}.is_some() && self.{}.is_full() {{\n            return Attempt::Blocked;\n        }}\n",
        buffer(receiver)
    )
}

/// The guard for a receiver several routes end at: the messages are counted,
/// and there must be room for all of them.
fn room_for_several(receiver: &Target, messages: &[usize]) -> String {
    let count = format!("to_{}", owner(receiver));
    let counted: String = messages
        .iter()
        .map(|i| format!("        if m{i}.is_some() {{\n            {count} += 1;\n        }}\n"))
        .collect();
    format!(
        "        let mut {count} = 0;\n{counted}        if self.{}.len() + {count} > {}_CAPACITY {{\n            return Attempt::Blocked;\n        }}\n",
        buffer(receiver),
        upper(owner(receiver))
    )
}

/// The delivery itself: the message leaves the mailbox, the machine takes its
/// new state, and every routed message is appended to its receiver.
fn commit(leaf: &Leaf) -> String {
    let id = &leaf.id;
    let pushes: String = leaf
        .routes
        .iter()
        .enumerate()
        .map(|(i, route)| {
            format!(
                "        if let Some(message) = m{i} {{\n            let _ = self.{}.push(message);\n        }}\n",
                buffer(&route.target)
            )
        })
        .collect();
    format!(
        "        let _ = self.{}.pop();\n        self.{} = state;\n{pushes}        Attempt::Delivered\n    }}\n\n",
        composite_v2::Machine(id).inbox(),
        composite_v2::Machine(id).state()
    )
}

/// The field of the buffer a route's messages go to.
fn buffer(target: &Target) -> String {
    match target {
        Target::Leaf(id) => composite_v2::Machine(id).inbox(),
        Target::Sink(port) => composite_v2::OutPort(port).outbox(),
    }
}

/// The machine or output port that owns the receiving buffer.
fn owner(target: &Target) -> &str {
    match target {
        Target::Leaf(id) | Target::Sink(id) => id,
    }
}
