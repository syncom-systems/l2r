//! The `System` a root `composite/v2` generates: the scheduler of the flat
//! network of every machine in the model.
//!
//! The code is shaped for proof, like `composite/v1`'s: mailboxes and outboxes
//! are fixed-size arrays with a length, and everything is straight-line code
//! with explicit matches (no loops outside `run`, no closures, no allocation).
//! The Lean model of this scheduler is `L2R.Async1.Ports` in
//! `lean/L2R/Async1Ports.lean`.

use l2r_validate::flat::{FlatRoute, HopKind, Leaf, Target};

use crate::PlanError;
use crate::text;
use crate::v2::Gen;

fn upper(id: &str) -> String {
    id.to_ascii_uppercase()
}

/// The part of the root module's documentation that states the behaviour.
pub fn behaviour_doc(generator: &Gen<'_>) -> String {
    let mut doc = String::from(
        "\n# The flat network\n\n\
         Hierarchy is structure; behaviour is that of the flat network of the\n\
         machines. Scheduler order (identifier order) and mailbox capacities:\n\n",
    );
    for (i, leaf) in generator.flat.leaves.iter().enumerate() {
        doc.push_str(&format!(
            "- {i}: `{}` (capacity {})\n",
            leaf.id, leaf.capacity
        ));
    }
    doc.push_str("\nWhere each machine's outputs go, in routing order:\n\n");
    for leaf in &generator.flat.leaves {
        for route in &leaf.routes {
            let via: Vec<String> = route.hops.iter().map(|h| format!("`{}`", h.id)).collect();
            let target = match &route.target {
                Target::Leaf(id) => format!("machine `{id}`"),
                Target::Sink(port) => format!("outbox `{port}`"),
            };
            doc.push_str(&format!(
                "- `{}` → {target} through {}\n",
                leaf.id,
                via.join(", ")
            ));
        }
    }
    doc.push_str("\nWhere each input port's messages go:\n\n");
    for input in &generator.flat.inputs {
        let via: Vec<String> = input.hops.iter().map(|h| format!("`{}`", h.id)).collect();
        doc.push_str(&format!(
            "- `{}` → machine `{}` through {}\n",
            input.port,
            input.target,
            via.join(", ")
        ));
    }
    doc.push_str(
        "\n# Behaviour\n\n\
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
         separately from this interface.\n",
    );
    doc
}

/// One bounded FIFO buffer type: a mailbox or an outbox.
struct Buffer {
    /// Struct name, e.g. `FilterInbox`.
    ty: String,
    /// Element type alias, e.g. `FilterInput`.
    element: String,
    /// Capacity constant, e.g. `FILTER_CAPACITY`.
    constant: String,
    capacity: usize,
    doc: String,
    /// Mailboxes are peeked by the scheduler; outboxes are only pushed and popped.
    peek: bool,
}

fn buffer(b: &Buffer) -> String {
    let Buffer {
        ty,
        element,
        constant,
        capacity: n,
        doc,
        peek,
    } = b;
    let peek = if *peek {
        format!("    fn peek(&self) -> Option<{element}> {{\n        self.slots[0]\n    }}\n\n")
    } else {
        String::new()
    };
    let nones = vec!["None"; *n].join(", ");
    let slots = if nones.len() + 20 <= 100 {
        format!("            slots: [{nones}],\n")
    } else {
        // rustfmt packs short array elements, as many per line as fit in 100 columns.
        let mut text = String::from("            slots: [\n");
        let mut line = String::new();
        for _ in 0..*n {
            if 16 + line.len() + "None,".len() > 100 {
                text.push_str(&format!("                {}\n", line.trim_end()));
                line.clear();
            }
            line.push_str("None, ");
        }
        text.push_str(&format!(
            "                {}\n            ],\n",
            line.trim_end()
        ));
        text
    };
    let mut shift = String::new();
    for i in 0..n.saturating_sub(1) {
        shift.push_str(&format!(
            "        self.slots[{i}] = self.slots[{}];\n",
            i + 1
        ));
    }
    format!(
        "/// {doc}\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub struct {ty} {{\n    slots: [Option<{element}>; {n}],\n    len: usize,\n}}\n\n\
         impl {ty} {{\n\
         \x20   fn new() -> Self {{\n        Self {{\n{slots}            len: 0,\n        }}\n    }}\n\n\
         \x20   /// Number of messages held.\n    pub fn len(&self) -> usize {{\n        self.len\n    }}\n\n\
         \x20   /// True when no message is held.\n    pub fn is_empty(&self) -> bool {{\n        self.len == 0\n    }}\n\n\
         \x20   /// True when at capacity.\n    pub fn is_full(&self) -> bool {{\n        self.len >= {constant}\n    }}\n\n\
         \x20   /// The message at `index`, counting from the oldest.\n    pub fn get(&self, index: usize) -> Option<{element}> {{\n        if index >= self.len {{\n            return None;\n        }}\n        self.slots[index]\n    }}\n\n\
         {peek}\
         \x20   /// Appends `message`; returns `false`, changing nothing, when full.\n    fn push(&mut self, message: {element}) -> bool {{\n        if self.len >= {constant} {{\n            return false;\n        }}\n        self.slots[self.len] = Some(message);\n        self.len += 1;\n        true\n    }}\n\n\
         \x20   /// Removes and returns the oldest message.\n    fn pop(&mut self) -> Option<{element}> {{\n        if self.len == 0 {{\n            return None;\n        }}\n        let first = self.slots[0];\n{shift}        self.slots[{last}] = None;\n        self.len -= 1;\n        first\n    }}\n}}\n\n",
        last = n - 1,
    )
}

/// The field of the buffer a route's messages go to.
fn target_buffer(target: &Target) -> String {
    match target {
        Target::Leaf(id) => format!("{id}_inbox"),
        Target::Sink(port) => format!("{port}_outbox"),
    }
}

fn target_id(target: &Target) -> &str {
    match target {
        Target::Leaf(id) | Target::Sink(id) => id,
    }
}

fn target_element(target: &Target) -> String {
    match target {
        Target::Leaf(id) => format!("{}Input", text::type_name(id)),
        Target::Sink(port) => format!("{}Message", text::type_name(port)),
    }
}

/// Everything in the root module after the `mod` declarations.
pub fn system(generator: &Gen<'_>) -> Result<String, PlanError> {
    let flat = generator.flat;
    let model = generator.model;
    let mut out = String::new();

    // Type aliases: short, unique names for every machine's and port's types.
    for leaf in &flat.leaves {
        let ty = text::type_name(&leaf.id);
        let module = generator.machine_module(&leaf.id);
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
    }
    for input in &flat.inputs {
        out.push_str(&format!(
            "/// Messages the input port `{}` accepts; mirrors `{}`.\npub type {}Message = {};\n\n",
            input.port,
            input.message,
            text::type_name(&input.port),
            generator.type_path(&input.message)?
        ));
    }
    for sink in &flat.sinks {
        out.push_str(&format!(
            "/// Messages the output port `{}` emits; mirrors `{}`.\npub type {}Message = {};\n\n",
            sink.port,
            sink.message,
            text::type_name(&sink.port),
            generator.type_path(&sink.message)?
        ));
    }
    for leaf in &flat.leaves {
        out.push_str(&format!(
            "/// Capacity of `{}`'s mailbox, from the architecture model.\npub const {}_CAPACITY: usize = {};\n",
            leaf.id,
            upper(&leaf.id),
            leaf.capacity
        ));
    }
    for sink in &flat.sinks {
        out.push_str(&format!(
            "/// Capacity of the outbox of port `{}`, from the architecture model.\npub const {}_CAPACITY: usize = {};\n",
            sink.port,
            upper(&sink.port),
            sink.capacity
        ));
    }
    out.push('\n');
    for leaf in &flat.leaves {
        let ty = text::type_name(&leaf.id);
        out.push_str(&buffer(&Buffer {
            ty: format!("{ty}Inbox"),
            element: format!("{ty}Input"),
            constant: format!("{}_CAPACITY", upper(&leaf.id)),
            capacity: leaf.capacity as usize,
            doc: format!(
                "The bounded FIFO mailbox of `{}`: at most `{}_CAPACITY` messages, oldest first.",
                leaf.id,
                upper(&leaf.id)
            ),
            peek: true,
        }));
    }
    for sink in &flat.sinks {
        let ty = text::type_name(&sink.port);
        out.push_str(&buffer(&Buffer {
            ty: format!("{ty}Outbox"),
            element: format!("{ty}Message"),
            constant: format!("{}_CAPACITY", upper(&sink.port)),
            capacity: sink.capacity as usize,
            doc: format!(
                "The bounded FIFO outbox of port `{}`: at most `{}_CAPACITY` messages, oldest first.",
                sink.port,
                upper(&sink.port)
            ),
            peek: false,
        }));
    }
    out.push_str(
        "/// A port refused a message because its target's mailbox is full. The caller\n\
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
         enum Attempt {\n    Delivered,\n    Blocked,\n    Idle,\n}\n\n",
    );
    // Route functions: the translations a message passes on one route, composed.
    for leaf in &flat.leaves {
        for (i, route) in leaf.routes.iter().enumerate() {
            out.push_str(&route_fn(generator, leaf, i, route));
        }
    }

    out.push_str(
        "/// The composed system: one owned state and one bounded mailbox per machine,\n\
         /// and one bounded outbox per output port.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct System {\n",
    );
    for leaf in &flat.leaves {
        let ty = text::type_name(&leaf.id);
        out.push_str(&format!(
            "    {id}_state: {ty}State,\n    {id}_inbox: {ty}Inbox,\n",
            id = leaf.id
        ));
    }
    for sink in &flat.sinks {
        out.push_str(&format!(
            "    {}_outbox: {}Outbox,\n",
            sink.port,
            text::type_name(&sink.port)
        ));
    }
    out.push_str("    next: usize,\n}\n\n");
    out.push_str(&format!(
        "impl System {{\n    const MACHINES: usize = {};\n\n",
        flat.leaves.len()
    ));
    out.push_str(
        "    /// Every machine in its initial state, with empty mailboxes and outboxes.\n    pub fn new() -> Self {\n        Self {\n",
    );
    for leaf in &flat.leaves {
        out.push_str(&format!(
            "            {id}_state: {module}::init(),\n            {id}_inbox: {ty}Inbox::new(),\n",
            id = leaf.id,
            module = generator.machine_module(&leaf.id),
            ty = text::type_name(&leaf.id)
        ));
    }
    for sink in &flat.sinks {
        out.push_str(&format!(
            "            {}_outbox: {}Outbox::new(),\n",
            sink.port,
            text::type_name(&sink.port)
        ));
    }
    out.push_str("            next: 0,\n        }\n    }\n\n");
    for leaf in &flat.leaves {
        let ty = text::type_name(&leaf.id);
        out.push_str(&format!(
            "    /// Current state of `{id}`. Read-only: only its own `step` changes it.\n    pub fn {id}_state(&self) -> {ty}State {{\n        self.{id}_state\n    }}\n\n\
             \x20   /// The mailbox of `{id}`, read-only.\n    pub fn {id}_inbox(&self) -> &{ty}Inbox {{\n        &self.{id}_inbox\n    }}\n\n",
            id = leaf.id
        ));
    }
    for sink in &flat.sinks {
        out.push_str(&format!(
            "    /// The outbox of port `{id}`, read-only.\n    pub fn {id}_outbox(&self) -> &{ty}Outbox {{\n        &self.{id}_outbox\n    }}\n\n",
            id = sink.port,
            ty = text::type_name(&sink.port)
        ));
    }
    for input in &flat.inputs {
        let ty = text::type_name(&input.port);
        out.push_str(&format!(
            "    /// Port `{id}`: offers `message` to `{to}`. Accepted only if its mailbox has room;\n    /// otherwise `Err(Rejected)`, and the caller keeps the message.\n    pub fn send_{id}(&mut self, message: {ty}Message) -> Result<(), Rejected> {{\n",
            id = input.port,
            to = input.target
        ));
        let last = input.hops.len().saturating_sub(1);
        for (i, hop) in input.hops.iter().enumerate() {
            let name = if i == last { "input" } else { "message" };
            out.push_str(&format!(
                "        let {name} = {}(message);\n",
                generator.hop_function(hop)
            ));
        }
        out.push_str(&format!(
            "        if self.{}_inbox.push(input) {{\n            Ok(())\n        }} else {{\n            Err(Rejected)\n        }}\n    }}\n\n",
            input.target
        ));
    }
    for sink in &flat.sinks {
        out.push_str(&format!(
            "    /// Port `{id}`: removes and returns the oldest message the system has emitted\n    /// there, if any. Taking a message makes room: a sender blocked by this outbox\n    /// can then proceed.\n    pub fn take_{id}(&mut self) -> Option<{ty}Message> {{\n        self.{id}_outbox.pop()\n    }}\n\n",
            id = sink.port,
            ty = text::type_name(&sink.port)
        ));
    }
    out.push_str(
        "    /// True when no mailbox holds a message. Outboxes may still hold messages\n    /// for the environment.\n    pub fn is_settled(&self) -> bool {\n        let mut settled = true;\n",
    );
    for leaf in &flat.leaves {
        out.push_str(&format!(
            "        settled &= self.{}_inbox.is_empty();\n",
            leaf.id
        ));
    }
    out.push_str("        settled\n    }\n\n");
    out.push_str(
        "    /// Serves the next machine, in round-robin order, whose delivery is enabled.\n",
    );
    if flat.leaves.len() == 1 {
        // One machine: the round-robin arithmetic is kept, so that the code has
        // the same shape as its Lean model for every number of machines.
        out.push_str("    #[allow(clippy::modulo_one)]\n");
    }
    out.push_str(
        "    pub fn step_once(&mut self) -> Progress {\n        let mut blocked = false;\n",
    );
    for offset in 0..flat.leaves.len() {
        let position = if offset == 0 {
            "self.next".to_string()
        } else {
            format!("self.next + {offset}")
        };
        out.push_str(&format!(
            "        let machine = ({position}) % Self::MACHINES;\n        match self.attempt(machine) {{\n            Attempt::Delivered => {{\n                self.next = (machine + 1) % Self::MACHINES;\n                return Progress::Delivered;\n            }}\n            Attempt::Blocked => blocked = true,\n            Attempt::Idle => {{}}\n        }}\n"
        ));
    }
    out.push_str(
        "        if blocked {\n            Progress::Blocked\n        } else {\n            Progress::Settled\n        }\n    }\n\n\
         \x20   /// Delivers until settled, blocked or `max_steps` deliveries.\n    pub fn run(&mut self, max_steps: usize) -> Result<usize, Stopped> {\n        for steps in 0..max_steps {\n            match self.step_once() {\n                Progress::Delivered => {}\n                Progress::Settled => return Ok(steps),\n                Progress::Blocked => return Err(Stopped::Blocked { steps }),\n            }\n        }\n        if self.is_settled() {\n            Ok(max_steps)\n        } else {\n            Err(Stopped::Budget { steps: max_steps })\n        }\n    }\n\n\
         \x20   fn attempt(&mut self, machine: usize) -> Attempt {\n        match machine {\n",
    );
    for (index, leaf) in flat.leaves.iter().enumerate() {
        out.push_str(&format!(
            "            {index} => self.attempt_{}(),\n",
            leaf.id
        ));
    }
    out.push_str("            _ => Attempt::Idle,\n        }\n    }\n\n");
    for leaf in &flat.leaves {
        out.push_str(&attempt_fn(generator, leaf));
    }
    out.truncate(out.trim_end().len());
    out.push_str("\n}\n\nimpl Default for System {\n    fn default() -> Self {\n        Self::new()\n    }\n}\n");
    let _ = model;
    Ok(out)
}

/// The function composing the translations of one route.
fn route_fn(generator: &Gen<'_>, leaf: &Leaf, index: usize, route: &FlatRoute) -> String {
    let ty = text::type_name(&leaf.id);
    let via: Vec<String> = route.hops.iter().map(|h| format!("`{}`", h.id)).collect();
    let target = match &route.target {
        Target::Leaf(id) => format!("machine `{id}`"),
        Target::Sink(port) => format!("outbox `{port}`"),
    };
    let mut needs_allow = false;
    let body = route_expr(generator, &route.hops, "output", "    ", &mut needs_allow);
    let allow = if needs_allow {
        "#[allow(clippy::manual_map)]\n"
    } else {
        ""
    };
    format!(
        "/// Route {index} of `{id}`: to {target} through {via}.\n\
         {allow}fn {id}_route_{index}(output: {ty}Output) -> Option<{element}> {{\n    {body}\n}}\n\n",
        id = leaf.id,
        via = via.join(", "),
        element = target_element(&route.target),
    )
}

/// The expression that carries `value` through `hops`. An input port is total,
/// so it wraps the value; a message edge is matched on, and `None` ends the route.
fn route_expr(
    generator: &Gen<'_>,
    hops: &[l2r_validate::flat::Hop],
    value: &str,
    indent: &str,
    needs_allow: &mut bool,
) -> String {
    let Some((hop, rest)) = hops.split_first() else {
        return value.to_string();
    };
    let call = format!("{}({value})", generator.hop_function(hop));
    match (hop.kind, rest.is_empty()) {
        (HopKind::Edge, true) => call,
        (HopKind::Port, true) => {
            *needs_allow = true;
            format!("Some({call})")
        }
        (HopKind::Port, false) => route_expr(generator, rest, &call, indent, needs_allow),
        (HopKind::Edge, false) => {
            let inner = format!("{indent}    ");
            format!(
                "match {call} {{\n{inner}Some(message) => {},\n{inner}None => None,\n{indent}}}",
                route_expr(generator, rest, "message", &inner, needs_allow)
            )
        }
    }
}

/// `attempt_<machine>`: one guarded delivery.
fn attempt_fn(generator: &Gen<'_>, leaf: &Leaf) -> String {
    let id = &leaf.id;
    let module = generator.machine_module(id);
    let mut out = format!(
        "    fn attempt_{id}(&mut self) -> Attempt {{\n        let input = match self.{id}_inbox.peek() {{\n            Some(input) => input,\n            None => return Attempt::Idle,\n        }};\n        let (state, output) = {module}::step(self.{id}_state, input);\n"
    );
    for (i, _) in leaf.routes.iter().enumerate() {
        out.push_str(&format!(
            "        let m{i} = match output {{\n            Some(output) => {id}_route_{i}(output),\n            None => None,\n        }};\n"
        ));
    }
    // Room: every receiver must have room for everything this delivery sends it.
    let mut receivers: Vec<&Target> = Vec::new();
    for route in &leaf.routes {
        if !receivers.contains(&&route.target) {
            receivers.push(&route.target);
        }
    }
    for receiver in &receivers {
        let messages: Vec<usize> = leaf
            .routes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.target == **receiver)
            .map(|(i, _)| i)
            .collect();
        let buffer = target_buffer(receiver);
        if let [only] = messages[..] {
            out.push_str(&format!(
                "        if m{only}.is_some() && self.{buffer}.is_full() {{\n            return Attempt::Blocked;\n        }}\n"
            ));
        } else {
            let count = format!("to_{}", target_id(receiver));
            out.push_str(&format!("        let mut {count} = 0;\n"));
            for i in &messages {
                out.push_str(&format!(
                    "        if m{i}.is_some() {{\n            {count} += 1;\n        }}\n"
                ));
            }
            out.push_str(&format!(
                "        if self.{buffer}.len() + {count} > {}_CAPACITY {{\n            return Attempt::Blocked;\n        }}\n",
                upper(target_id(receiver))
            ));
        }
    }
    out.push_str(&format!(
        "        let _ = self.{id}_inbox.pop();\n        self.{id}_state = state;\n"
    ));
    for (i, route) in leaf.routes.iter().enumerate() {
        out.push_str(&format!(
            "        if let Some(message) = m{i} {{\n            let _ = self.{}.push(message);\n        }}\n",
            target_buffer(&route.target)
        ));
    }
    out.push_str("        Attempt::Delivered\n    }\n\n");
    out
}
