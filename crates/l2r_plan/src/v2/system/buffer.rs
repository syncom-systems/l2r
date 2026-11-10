//! The bounded FIFO buffer types of the `System`: one mailbox type per machine
//! and one outbox type per output port of the root.

use l2r_validate::flat::{Flat, Leaf, Sink};

use super::{per_holder, upper};
use crate::text;

/// Every mailbox type, then every outbox type.
pub(super) fn buffers(flat: &Flat) -> String {
    per_holder(
        flat,
        |leaf| Buffer::mailbox(leaf).definition(),
        |sink| Buffer::outbox(sink).definition(),
    )
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

/// Whose messages a buffer holds.
#[derive(Clone, Copy, PartialEq)]
enum BufferKind {
    /// The messages waiting for a machine.
    Mailbox,
    /// The messages waiting for the environment at a root output port.
    Outbox,
}

impl BufferKind {
    fn type_suffix(self) -> &'static str {
        match self {
            BufferKind::Mailbox => "Inbox",
            BufferKind::Outbox => "Outbox",
        }
    }

    fn element_suffix(self) -> &'static str {
        match self {
            BufferKind::Mailbox => "Input",
            BufferKind::Outbox => "Message",
        }
    }

    /// As the buffer's documentation names it, before the holder.
    fn described(self) -> &'static str {
        match self {
            BufferKind::Mailbox => "mailbox of",
            BufferKind::Outbox => "outbox of port",
        }
    }

    /// Mailboxes are peeked by the scheduler; outboxes are only pushed and popped.
    fn is_peeked(self) -> bool {
        self == BufferKind::Mailbox
    }
}

impl Buffer {
    fn mailbox(leaf: &Leaf) -> Self {
        Self::of(BufferKind::Mailbox, &leaf.id, leaf.capacity as usize)
    }

    fn outbox(sink: &Sink) -> Self {
        Self::of(BufferKind::Outbox, &sink.port, sink.capacity as usize)
    }

    /// The buffer of `holder`: a machine for a mailbox, a root output port
    /// for an outbox.
    fn of(kind: BufferKind, holder: &str, capacity: usize) -> Self {
        let ty = text::type_name(holder);
        let constant = format!("{}_CAPACITY", upper(holder));
        Self {
            ty: format!("{ty}{}", kind.type_suffix()),
            element: format!("{ty}{}", kind.element_suffix()),
            doc: format!(
                "The bounded FIFO {} `{holder}`: at most `{constant}` messages, oldest first.",
                kind.described()
            ),
            constant,
            capacity,
            peek: kind.is_peeked(),
        }
    }

    /// The struct and its `impl`.
    fn definition(&self) -> String {
        let Buffer {
            ty,
            element,
            constant,
            capacity: n,
            doc,
            ..
        } = self;
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
            slots = self.empty_slots(),
            peek = self.peek_fn(),
            shift = self.shift_down(),
            last = n - 1,
        )
    }

    /// `peek`, for a buffer the scheduler peeks.
    fn peek_fn(&self) -> String {
        if !self.peek {
            return String::new();
        }
        format!(
            "    fn peek(&self) -> Option<{}> {{\n        self.slots[0]\n    }}\n\n",
            self.element
        )
    }

    /// The `slots` field of the empty buffer, laid out as rustfmt would.
    fn empty_slots(&self) -> String {
        let nones = vec!["None"; self.capacity].join(", ");
        if nones.len() + 20 <= 100 {
            return format!("            slots: [{nones}],\n");
        }
        format!(
            "            slots: [\n{}            ],\n",
            packed_nones(self.capacity)
        )
    }

    /// The assignments of `pop` that move every later message one slot down.
    fn shift_down(&self) -> String {
        (0..self.capacity.saturating_sub(1))
            .map(|i| format!("        self.slots[{i}] = self.slots[{}];\n", i + 1))
            .collect()
    }
}

/// `n` array elements `None`, on lines of their own: rustfmt packs short array
/// elements, as many per line as fit in 100 columns.
fn packed_nones(n: usize) -> String {
    let mut text = String::new();
    let mut line = String::new();
    for _ in 0..n {
        if 16 + line.len() + "None,".len() > 100 {
            text.push_str(&format!("                {}\n", line.trim_end()));
            line.clear();
        }
        line.push_str("None, ");
    }
    text.push_str(&format!("                {}\n", line.trim_end()));
    text
}
