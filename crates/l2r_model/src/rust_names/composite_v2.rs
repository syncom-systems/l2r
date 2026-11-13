//! The names of the `composite/v2` scheduler (`System` in the root's module)
//! and of every `routes.rs`. Frozen with the profile.
//!
//! Names are derived from a [`Machine`], an [`InPort`], an [`OutPort`] or an
//! [`Edge`], so an id cannot be passed where another kind is meant. They are not a
//! checked name set as for `composite/v0` and `composite/v1`: rule S020,
//! which rejects their conflicts, is a published rule of schema `v2` whose
//! diagnostics are frozen, and it asks these types for the names instead of
//! restating them.

use super::type_name;

/// Members of `System` the profile names itself. S020 treats the fields and
/// methods of `System` as one set of members.
pub const SYSTEM_MEMBERS: &[&str] = &["attempt", "is_settled", "new", "next", "run", "step_once"];

/// A machine of the flat network, by component id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Machine<'a>(pub &'a str);

/// An input port, by port id: the environment, or the composite above, sends
/// through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InPort<'a>(pub &'a str);

/// An output port of the root, by port id: the environment takes from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutPort<'a>(pub &'a str);

/// A message edge, by edge id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge<'a>(pub &'a str);

impl Machine<'_> {
    /// The field and accessor holding the machine's state.
    pub fn state(self) -> String {
        format!("{}_state", self.0)
    }

    /// The field and accessor holding the machine's mailbox.
    pub fn inbox(self) -> String {
        format!("{}_inbox", self.0)
    }

    /// The method attempting one delivery to the machine.
    pub fn attempt(self) -> String {
        format!("attempt_{}", self.0)
    }

    /// The `System` members generated for the machine, in S020's order.
    pub fn members(self) -> Vec<String> {
        vec![self.state(), self.inbox(), self.attempt()]
    }
}

impl InPort<'_> {
    /// The method through which the environment sends to a root input port.
    pub fn send(self) -> String {
        format!("send_{}", self.0)
    }

    /// The `routes.rs` function implementing the port's translation.
    pub fn function(self) -> String {
        format!("port_{}", self.0)
    }

    /// The `System` members generated for a root input port.
    pub fn members(self) -> Vec<String> {
        vec![self.send()]
    }
}

impl OutPort<'_> {
    /// The field and accessor holding the port's outbox.
    pub fn outbox(self) -> String {
        format!("{}_outbox", self.0)
    }

    /// The method through which the environment takes from the port.
    pub fn take(self) -> String {
        format!("take_{}", self.0)
    }

    /// The `System` members generated for a root output port.
    pub fn members(self) -> Vec<String> {
        vec![self.outbox(), self.take()]
    }
}

impl Edge<'_> {
    /// The `routes.rs` function implementing the edge: the edge's own id.
    pub fn function(self) -> String {
        self.0.to_string()
    }
}

/// The prefix of the types generated for the machine or root port `id`
/// (`<Prefix>State`, `<Prefix>Inbox`, `<Prefix>Message`, `<Prefix>Outbox`).
pub fn type_prefix(id: &str) -> String {
    type_name(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names() {
        assert_eq!(
            Machine("pump").members(),
            ["pump_state", "pump_inbox", "attempt_pump"]
        );
        assert_eq!(OutPort("alarm").members(), ["alarm_outbox", "take_alarm"]);
        assert_eq!(InPort("start").members(), ["send_start"]);
        assert_eq!(InPort("start").function(), "port_start");
        assert_eq!(Edge("to_valve").function(), "to_valve");
        assert_eq!(type_prefix("tank_level"), "TankLevel");
    }
}
