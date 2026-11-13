//! The `System` a root `composite/v2` generates: the scheduler of the flat
//! network of every machine in the model.
//!
//! The code is shaped for proof, like `composite/v1`'s: mailboxes and outboxes
//! are fixed-size arrays with a length, and everything is straight-line code
//! with explicit matches (no loops outside `run`, no closures, no allocation).
//! The Lean model of this scheduler is `L2R.Async1.Ports` in
//! `lean/L2R/Async1Ports.lean`.

mod attempt;
mod buffer;
mod route;

use l2r_model::rust_names::composite_v2;
use l2r_validate::flat::{Flat, FlatInput, Hop, Leaf, Sink, Target};

use crate::PlanError;
use crate::v2::Gen;

fn upper(id: &str) -> String {
    id.to_ascii_uppercase()
}

/// The edges and ports a message passes, in order, as the documentation lists them.
fn via(hops: &[Hop]) -> String {
    let ids: Vec<String> = hops.iter().map(|h| format!("`{}`", h.id)).collect();
    ids.join(", ")
}

/// A route's receiver, as the documentation names it.
fn receiver(target: &Target) -> String {
    match target {
        Target::Leaf(id) => format!("machine `{id}`"),
        Target::Sink(port) => format!("outbox `{port}`"),
    }
}

/// Text for everything that holds messages: what `machine` gives for each
/// machine (its mailbox), in scheduler order, then what `outbox` gives for each
/// output port of the root. Constants, types, fields and accessors all follow
/// this one order.
fn per_holder(
    flat: &Flat,
    machine: impl Fn(&Leaf) -> String,
    outbox: impl Fn(&Sink) -> String,
) -> String {
    let machines = flat.leaves.iter().map(machine);
    machines.chain(flat.sinks.iter().map(outbox)).collect()
}

/// The part of the root module's documentation that states the behaviour.
pub fn behaviour_doc(generator: &Gen<'_>) -> String {
    let flat = generator.flat;
    [
        "\n# The flat network\n\n\
         Hierarchy is structure; behaviour is that of the flat network of the\n\
         machines. Scheduler order (identifier order) and mailbox capacities:\n\n",
        &scheduler_order(flat),
        "\nWhere each machine's outputs go, in routing order:\n\n",
        &output_routes(flat),
        "\nWhere each input port's messages go:\n\n",
        &input_routes(flat),
        BEHAVIOUR,
    ]
    .concat()
}

/// The machines by scheduler index, with their mailbox capacities.
fn scheduler_order(flat: &Flat) -> String {
    flat.leaves
        .iter()
        .enumerate()
        .map(|(i, leaf)| format!("- {i}: `{}` (capacity {})\n", leaf.id, leaf.capacity))
        .collect()
}

/// Every route of every machine: its receiver and what it passes.
fn output_routes(flat: &Flat) -> String {
    flat.leaves
        .iter()
        .flat_map(|leaf| {
            leaf.routes.iter().map(move |route| {
                format!(
                    "- `{}` → {} through {}\n",
                    leaf.id,
                    receiver(&route.target),
                    via(&route.hops)
                )
            })
        })
        .collect()
}

/// Every input port of the root: the machine it reaches and what it passes.
fn input_routes(flat: &Flat) -> String {
    flat.inputs
        .iter()
        .map(|input| {
            format!(
                "- `{}` → machine `{}` through {}\n",
                input.port,
                input.target,
                via(&input.hops)
            )
        })
        .collect()
}

const BEHAVIOUR: &str = "\n# Behaviour\n\n\
         Each machine's state is owned by that machine and changed only by its own\n\
         `step`. Messages travel through bounded FIFO mailboxes; what the system\n\
         tells the environment waits in bounded FIFO outboxes until the environment\n\
         takes it (`System::take_<port>`). A delivery is performed only if every\n\
         receiving mailbox and outbox has room for what it produces; otherwise\n\
         nothing changes and the sender is *blocked*. `System::step_once` reports\n\
         `Progress::Delivered`, `Progress::Settled` (no mailbox holds a message) or\n\
         `Progress::Blocked` (messages pending, no delivery enabled). A port send\n\
         returns `Err(Rejected)` when the target's mailbox is full; the caller keeps\n\
         the message. No message is dropped for lack of room. An edge translation\n\
         that returns `None` is the Lean specification's own decision that a message\n\
         means nothing to the receiver.\n\n\
         The scheduler serves machines round-robin in identifier order. This is one\n\
         deterministic scheduler; the Lean semantics permit every interleaving,\n\
         including the environment taking from an outbox at any time. When events\n\
         may arrive is an assumption about the environment, stated and verified\n\
         separately from this interface.\n";

/// Everything in the root module after the `mod` declarations.
pub fn system(generator: &Gen<'_>) -> Result<String, PlanError> {
    let flat = generator.flat;
    Ok([
        type_aliases(generator)?,
        capacity_constants(flat),
        buffer::buffers(flat),
        OUTCOMES.to_string(),
        route::route_functions(generator),
        system_struct(flat),
        system_impl(generator),
        DEFAULT_IMPL.to_string(),
    ]
    .concat())
}

/// Type aliases: short, unique names for every machine's and port's types.
fn type_aliases(generator: &Gen<'_>) -> Result<String, PlanError> {
    let flat = generator.flat;
    let mut out: String = flat
        .leaves
        .iter()
        .map(|leaf| machine_aliases(generator, leaf))
        .collect();
    for input in &flat.inputs {
        out.push_str(&input_alias(generator, input)?);
    }
    for sink in &flat.sinks {
        out.push_str(&output_alias(generator, sink)?);
    }
    Ok(out)
}

/// `<Machine>State`, `<Machine>Input` and `<Machine>Output`.
fn machine_aliases(generator: &Gen<'_>, leaf: &Leaf) -> String {
    let ty = composite_v2::type_prefix(&leaf.id);
    let module = generator.machine_module(&leaf.id);
    let mut out = String::new();
    for (suffix, what) in [
        ("State", "State"),
        ("Input", "Inputs"),
        ("Output", "Outputs"),
    ] {
        out.push_str(&format!(
            "/// {what} of `{}`.\npub type {ty}{suffix} = {module}::{suffix};\n",
            leaf.id
        ));
    }
    out.push('\n');
    out
}

fn input_alias(generator: &Gen<'_>, input: &FlatInput) -> Result<String, PlanError> {
    let carried = format!("the input port `{}` accepts", input.port);
    message_alias(generator, &input.port, &input.message, &carried)
}

fn output_alias(generator: &Gen<'_>, sink: &Sink) -> Result<String, PlanError> {
    let carried = format!("the output port `{}` emits", sink.port);
    message_alias(generator, &sink.port, &sink.message, &carried)
}

/// `<Port>Message`: the mirror of the Lean type of the messages a root port carries.
fn message_alias(
    generator: &Gen<'_>,
    port: &str,
    message: &str,
    carried: &str,
) -> Result<String, PlanError> {
    Ok(format!(
        "/// Messages {carried}; mirrors `{message}`.\npub type {}Message = {};\n\n",
        composite_v2::type_prefix(port),
        generator.type_path(message)?
    ))
}

/// `<MACHINE>_CAPACITY` for every mailbox, `<PORT>_CAPACITY` for every outbox.
fn capacity_constants(flat: &Flat) -> String {
    let mut out = per_holder(
        flat,
        |leaf| {
            format!(
                "/// Capacity of `{}`'s mailbox, from the architecture model.\npub const {}_CAPACITY: usize = {};\n",
                leaf.id,
                upper(&leaf.id),
                leaf.capacity
            )
        },
        |sink| {
            format!(
                "/// Capacity of the outbox of port `{}`, from the architecture model.\npub const {}_CAPACITY: usize = {};\n",
                sink.port,
                upper(&sink.port),
                sink.capacity
            )
        },
    );
    out.push('\n');
    out
}

/// `Rejected`, `Progress`, `Stopped` and the private `Attempt`: what sending,
/// stepping and running report.
const OUTCOMES: &str = "/// A port refused a message because its target's mailbox is full. The caller\n\
         /// keeps the message; nothing was changed or lost.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct Rejected;\n\n\
         /// What one call of `System::step_once` did.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Progress {\n\
         \x20   /// One message was delivered.\n    Delivered,\n\
         \x20   /// No mailbox holds a message.\n    Settled,\n\
         \x20   /// Messages are pending, but every delivery is blocked by a full mailbox or\n\
         \x20   /// by an outbox the environment has not emptied.\n    Blocked,\n}\n\n\
         /// `System::run` stopped before the system settled.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Stopped {\n\
         \x20   /// The step budget ran out before the system settled.\n    Budget {\n        /// Deliveries performed.\n        steps: usize,\n    },\n\
         \x20   /// Messages are pending but every delivery is blocked.\n    Blocked {\n        /// Deliveries performed before blocking.\n        steps: usize,\n    },\n}\n\n\
         enum Attempt {\n    Delivered,\n    Blocked,\n    Idle,\n}\n\n";

/// `struct System`: a state and a mailbox per machine, an outbox per output port.
fn system_struct(flat: &Flat) -> String {
    let fields = per_holder(
        flat,
        |leaf| {
            format!(
                "    {state}: {ty}State,\n    {inbox}: {ty}Inbox,\n",
                state = composite_v2::Machine(&leaf.id).state(),
                inbox = composite_v2::Machine(&leaf.id).inbox(),
                ty = composite_v2::type_prefix(&leaf.id)
            )
        },
        |sink| {
            format!(
                "    {}: {}Outbox,\n",
                composite_v2::Port(&sink.port).outbox(),
                composite_v2::type_prefix(&sink.port)
            )
        },
    );
    [
        "/// The composed system: one owned state and one bounded mailbox per machine,\n\
         /// and one bounded outbox per output port.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct System {\n",
        &fields,
        "    next: usize,\n}\n\n",
    ]
    .concat()
}

/// `impl System`, section by section.
fn system_impl(generator: &Gen<'_>) -> String {
    let flat = generator.flat;
    let mut out = [
        format!(
            "impl System {{\n    const MACHINES: usize = {};\n\n",
            flat.leaves.len()
        ),
        constructor(generator),
        accessors(flat),
        sends(generator),
        takes(flat),
        is_settled(flat),
        step_once(flat),
        RUN.to_string(),
        attempt::dispatch(flat),
        attempt::attempts(generator),
    ]
    .concat();
    out.truncate(out.trim_end().len());
    out.push_str("\n}\n\n");
    out
}

/// `System::new`.
fn constructor(generator: &Gen<'_>) -> String {
    let initial = per_holder(
        generator.flat,
        |leaf| {
            format!(
                "            {state}: {module}::init(),\n            {inbox}: {ty}Inbox::new(),\n",
                state = composite_v2::Machine(&leaf.id).state(),
                inbox = composite_v2::Machine(&leaf.id).inbox(),
                module = generator.machine_module(&leaf.id),
                ty = composite_v2::type_prefix(&leaf.id)
            )
        },
        |sink| {
            format!(
                "            {}: {}Outbox::new(),\n",
                composite_v2::Port(&sink.port).outbox(),
                composite_v2::type_prefix(&sink.port)
            )
        },
    );
    [
        "    /// Every machine in its initial state, with empty mailboxes and outboxes.\n    pub fn new() -> Self {\n        Self {\n",
        &initial,
        "            next: 0,\n        }\n    }\n\n",
    ]
    .concat()
}

/// The read-only accessors: `<machine>_state`, `<machine>_inbox`, `<port>_outbox`.
fn accessors(flat: &Flat) -> String {
    per_holder(
        flat,
        |leaf| {
            format!(
                "    /// Current state of `{id}`. Read-only: only its own `step` changes it.\n    pub fn {state}(&self) -> {ty}State {{\n        self.{state}\n    }}\n\n\
                 \x20   /// The mailbox of `{id}`, read-only.\n    pub fn {inbox}(&self) -> &{ty}Inbox {{\n        &self.{inbox}\n    }}\n\n",
                id = leaf.id,
                state = composite_v2::Machine(&leaf.id).state(),
                inbox = composite_v2::Machine(&leaf.id).inbox(),
                ty = composite_v2::type_prefix(&leaf.id)
            )
        },
        |sink| {
            format!(
                "    /// The outbox of port `{id}`, read-only.\n    pub fn {outbox}(&self) -> &{ty}Outbox {{\n        &self.{outbox}\n    }}\n\n",
                id = sink.port,
                outbox = composite_v2::Port(&sink.port).outbox(),
                ty = composite_v2::type_prefix(&sink.port)
            )
        },
    )
}

/// `send_<port>` for every input port of the root.
fn sends(generator: &Gen<'_>) -> String {
    generator
        .flat
        .inputs
        .iter()
        .map(|input| send(generator, input))
        .collect()
}

/// `send_<port>`: the message is carried through the ports on its way down and
/// offered to the mailbox of the machine it reaches.
fn send(generator: &Gen<'_>, input: &FlatInput) -> String {
    let last = input.hops.len().saturating_sub(1);
    let translations: String = input
        .hops
        .iter()
        .enumerate()
        .map(|(i, hop)| {
            let name = if i == last { "input" } else { "message" };
            format!(
                "        let {name} = {}(message);\n",
                generator.hop_function(hop)
            )
        })
        .collect();
    format!(
        "    /// Port `{id}`: offers `message` to `{to}`. Accepted only if its mailbox has room;\n    /// otherwise `Err(Rejected)`, and the caller keeps the message.\n    pub fn {send}(&mut self, message: {ty}Message) -> Result<(), Rejected> {{\n\
         {translations}        if self.{target_inbox}.push(input) {{\n            Ok(())\n        }} else {{\n            Err(Rejected)\n        }}\n    }}\n\n",
        id = input.port,
        to = input.target,
        send = composite_v2::Port(&input.port).send(),
        target_inbox = composite_v2::Machine(&input.target).inbox(),
        ty = composite_v2::type_prefix(&input.port)
    )
}

/// `take_<port>` for every output port of the root.
fn takes(flat: &Flat) -> String {
    flat.sinks
        .iter()
        .map(|sink| {
            format!(
                "    /// Port `{id}`: removes and returns the oldest message the system has emitted\n    /// there, if any. Taking a message makes room: a sender blocked by this outbox\n    /// can then proceed.\n    pub fn {take}(&mut self) -> Option<{ty}Message> {{\n        self.{outbox}.pop()\n    }}\n\n",
                id = sink.port,
                take = composite_v2::Port(&sink.port).take(),
                outbox = composite_v2::Port(&sink.port).outbox(),
                ty = composite_v2::type_prefix(&sink.port)
            )
        })
        .collect()
}

/// `System::is_settled`.
fn is_settled(flat: &Flat) -> String {
    let mailboxes: String = flat
        .leaves
        .iter()
        .map(|leaf| {
            format!(
                "        settled &= self.{}.is_empty();\n",
                composite_v2::Machine(&leaf.id).inbox()
            )
        })
        .collect();
    [
        "    /// True when no mailbox holds a message. Outboxes may still hold messages\n    /// for the environment.\n    pub fn is_settled(&self) -> bool {\n        let mut settled = true;\n",
        &mailboxes,
        "        settled\n    }\n\n",
    ]
    .concat()
}

/// `System::step_once`: one try per machine, starting at `next`.
fn step_once(flat: &Flat) -> String {
    let machines = flat.leaves.len();
    // One machine: the round-robin arithmetic is kept, so that the code has
    // the same shape as its Lean model for every number of machines.
    let allow = if machines == 1 {
        "    #[allow(clippy::modulo_one)]\n"
    } else {
        ""
    };
    let tries: String = (0..machines).map(try_machine).collect();
    [
        "    /// Serves the next machine, in round-robin order, whose delivery is enabled.\n",
        allow,
        "    pub fn step_once(&mut self) -> Progress {\n        let mut blocked = false;\n",
        &tries,
        "        if blocked {\n            Progress::Blocked\n        } else {\n            Progress::Settled\n        }\n    }\n\n",
    ]
    .concat()
}

/// The try of the machine `offset` places after `next` in the round-robin order.
fn try_machine(offset: usize) -> String {
    let position = if offset == 0 {
        "self.next".to_string()
    } else {
        format!("self.next + {offset}")
    };
    format!(
        "        let machine = ({position}) % Self::MACHINES;\n        match self.attempt(machine) {{\n            Attempt::Delivered => {{\n                self.next = (machine + 1) % Self::MACHINES;\n                return Progress::Delivered;\n            }}\n            Attempt::Blocked => blocked = true,\n            Attempt::Idle => {{}}\n        }}\n"
    )
}

/// `System::run`.
const RUN: &str = "    /// Delivers until settled, blocked or `max_steps` deliveries.\n    pub fn run(&mut self, max_steps: usize) -> Result<usize, Stopped> {\n        for steps in 0..max_steps {\n            match self.step_once() {\n                Progress::Delivered => {}\n                Progress::Settled => return Ok(steps),\n                Progress::Blocked => return Err(Stopped::Blocked { steps }),\n            }\n        }\n        if self.is_settled() {\n            Ok(max_steps)\n        } else {\n            Err(Stopped::Budget { steps: max_steps })\n        }\n    }\n\n";

const DEFAULT_IMPL: &str =
    "impl Default for System {\n    fn default() -> Self {\n        Self::new()\n    }\n}\n";
