//! The names of the `composite/v2` scheduler (`System` in the root's module)
//! and of every `routes.rs`. Frozen with the profile.
//!
//! Names are derived from a [`Machine`], a [`Port`] or an [`Edge`], so a
//! port's id cannot be passed where a machine's is meant. They are not a
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

/// A port, by port id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port<'a>(pub &'a str);

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

impl Port<'_> {
    /// The field and accessor holding a root output port's outbox.
    pub fn outbox(self) -> String {
        format!("{}_outbox", self.0)
    }

    /// The method through which the environment sends to a root input port.
    pub fn send(self) -> String {
        format!("send_{}", self.0)
    }

    /// The method through which the environment takes from a root output port.
    pub fn take(self) -> String {
        format!("take_{}", self.0)
    }

    /// The `routes.rs` function implementing an input port.
    pub fn function(self) -> String {
        format!("port_{}", self.0)
    }

    /// The `System` members generated for a root port.
    pub fn members(self, is_out: bool) -> Vec<String> {
        if is_out {
            vec![self.outbox(), self.take()]
        } else {
            vec![self.send()]
        }
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
        assert_eq!(Port("alarm").members(true), ["alarm_outbox", "take_alarm"]);
        assert_eq!(Port("start").members(false), ["send_start"]);
        assert_eq!(Port("start").function(), "port_start");
        assert_eq!(Edge("to_valve").function(), "to_valve");
        assert_eq!(type_prefix("tank_level"), "TankLevel");
    }
}
