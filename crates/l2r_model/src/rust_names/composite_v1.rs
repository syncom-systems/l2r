//! The names of a `composite/v1` module (`mod.rs` and `routes.rs`). Frozen with
//! the profile.

use std::collections::BTreeMap;

use super::{Claims, Namespace, Owner, Problem, lean_last_segment, rust_type_name, type_name};

/// Fields of `System` the profile names itself.
pub const FIELDS: &[&str] = &["next"];
/// Methods and associated items of `impl System` the profile names itself.
pub const METHODS: &[&str] = &[
    "CHILDREN",
    "attempt",
    "is_settled",
    "new",
    "run",
    "step_once",
];
/// Types of the module the profile names itself.
pub const TYPES: &[&str] = &["Attempt", "Progress", "Rejected", "Stopped", "System"];

/// A child machine of the composite.
#[derive(Debug, Clone, Copy)]
pub struct Child<'a> {
    /// Component id.
    pub id: &'a str,
    /// Its `rust_module`.
    pub module: &'a str,
}

/// An input port of the composite.
#[derive(Debug, Clone, Copy)]
pub struct Port<'a> {
    /// Port id.
    pub id: &'a str,
    /// The qualified Lean type of its messages.
    pub message: &'a str,
}

/// Every name the generated module of one `composite/v1` uses, free of
/// conflicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemNames {
    children: BTreeMap<String, ChildNames>,
    ports: BTreeMap<String, PortNames>,
    fields: Vec<String>,
    methods: Vec<String>,
    types: Vec<String>,
    route_functions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChildNames {
    field: String,
    inbox: String,
    inbox_type: String,
    attempt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortNames {
    send: String,
    function: String,
    message_type: String,
}

impl SystemNames {
    /// The names generated for `children`, `ports` and `edges` (the ids of the
    /// approved message edges inside the composite). Ports that carry the same
    /// Lean type share its Rust type; distinct Lean types never do. `Err` lists
    /// every problem, not only the first.
    pub fn new(
        children: &[Child<'_>],
        ports: &[Port<'_>],
        edges: &[&str],
    ) -> Result<SystemNames, Vec<Problem>> {
        let mut claims = Claims::default();
        claims.claim_fixed(Namespace::Field, FIELDS);
        claims.claim_fixed(Namespace::Method, METHODS);
        claims.claim_fixed(Namespace::Type, TYPES);
        let mut named_children = BTreeMap::new();
        for child in children {
            let owner = Owner::Component(child.id.to_string());
            let names = ChildNames::of(child);
            for name in [&names.field, &names.inbox] {
                claims.claim(Namespace::Field, name, &owner);
                claims.claim(Namespace::Method, name, &owner);
            }
            claims.claim(Namespace::Method, &names.attempt, &owner);
            claims.claim(Namespace::Type, &names.inbox_type, &owner);
            named_children.insert(child.id.to_string(), names);
        }
        let mut named_ports = BTreeMap::new();
        for port in ports {
            let owner = Owner::Port(port.id.to_string());
            let names = PortNames::of(port);
            claims.claim(Namespace::Method, &names.send, &owner);
            claims.claim(Namespace::RouteFunction, &names.function, &owner);
            let lean = Owner::LeanType(port.message.to_string());
            match rust_type_name(lean_last_segment(port.message)) {
                Some(ty) => claims.claim(Namespace::Type, &ty, &lean),
                None => claims.reject(Namespace::Type, &names.message_type, &lean),
            }
            named_ports.insert(port.id.to_string(), names);
        }
        for edge in edges {
            claims.claim(
                Namespace::RouteFunction,
                edge,
                &Owner::Edge(edge.to_string()),
            );
        }
        let fields = claims.names(Namespace::Field);
        let methods = claims.names(Namespace::Method);
        let types = claims.names(Namespace::Type);
        let route_functions = claims.names(Namespace::RouteFunction);
        claims.finish(SystemNames {
            children: named_children,
            ports: named_ports,
            fields,
            methods,
            types,
            route_functions,
        })
    }

    fn child(&self, id: &str) -> &ChildNames {
        self.children
            .get(id)
            .unwrap_or_else(|| panic!("`{id}` is not a child of this composite"))
    }

    fn port(&self, id: &str) -> &PortNames {
        self.ports
            .get(id)
            .unwrap_or_else(|| panic!("`{id}` is not a port of this composite"))
    }

    /// The field holding child `id`'s state, and its read accessor.
    pub fn field(&self, id: &str) -> &str {
        &self.child(id).field
    }

    /// The field holding child `id`'s mailbox, and its read accessor.
    pub fn inbox(&self, id: &str) -> &str {
        &self.child(id).inbox
    }

    /// The type of child `id`'s mailbox.
    pub fn inbox_type(&self, id: &str) -> &str {
        &self.child(id).inbox_type
    }

    /// The method attempting one delivery to child `id`.
    pub fn attempt(&self, id: &str) -> &str {
        &self.child(id).attempt
    }

    /// The method through which the environment sends to port `id`.
    pub fn send(&self, id: &str) -> &str {
        &self.port(id).send
    }

    /// The function of `routes.rs` translating port `id`'s messages.
    pub fn port_function(&self, id: &str) -> &str {
        &self.port(id).function
    }

    /// The Rust type of port `id`'s messages.
    pub fn message_type(&self, id: &str) -> &str {
        &self.port(id).message_type
    }

    /// Every field of `System`, in order.
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Every method and associated item of `impl System`, in order.
    pub fn methods(&self) -> &[String] {
        &self.methods
    }

    /// Every type the module declares, in order.
    pub fn types(&self) -> &[String] {
        &self.types
    }

    /// Every function of `routes.rs`, in order.
    pub fn route_functions(&self) -> &[String] {
        &self.route_functions
    }
}

impl ChildNames {
    fn of(child: &Child<'_>) -> ChildNames {
        let m = child.module;
        ChildNames {
            field: m.to_string(),
            inbox: format!("{m}_inbox"),
            inbox_type: format!("{}Inbox", type_name(child.id)),
            attempt: format!("attempt_{m}"),
        }
    }
}

impl PortNames {
    fn of(port: &Port<'_>) -> PortNames {
        let id = port.id;
        PortNames {
            send: format!("send_{id}"),
            function: format!("port_{id}"),
            message_type: type_name(lean_last_segment(port.message)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child<'a>(id: &'a str, module: &'a str) -> Child<'a> {
        Child { id, module }
    }

    fn port<'a>(id: &'a str, message: &'a str) -> Port<'a> {
        Port { id, message }
    }

    fn example_children() -> Vec<Child<'static>> {
        vec![
            child("request_latch", "request_latch"),
            child("signal", "signal"),
        ]
    }

    #[test]
    fn the_example_names() {
        let names = SystemNames::new(
            &example_children(),
            &[
                port("button", "CrossingV1.Ports.ButtonEvent"),
                port("timer", "CrossingV1.Ports.TimerEvent"),
            ],
            &["latch_to_signal", "signal_to_latch"],
        )
        .unwrap();
        assert_eq!(names.inbox_type("request_latch"), "RequestLatchInbox");
        assert_eq!(names.attempt("signal"), "attempt_signal");
        assert_eq!(names.send("timer"), "send_timer");
        assert_eq!(names.port_function("timer"), "port_timer");
        assert_eq!(names.message_type("button"), "ButtonEvent");
    }

    #[test]
    fn ports_carrying_one_lean_type_share_its_rust_type() {
        let ports = [port("a", "M.Event"), port("b", "M.Event")];
        let names = SystemNames::new(&example_children(), &ports, &[]).unwrap();
        assert_eq!(names.message_type("a"), names.message_type("b"));
    }

    #[test]
    fn distinct_lean_types_never_share_a_rust_type() {
        let ports = [port("a", "A.Command"), port("b", "B.Command")];
        let problems = SystemNames::new(&example_children(), &ports, &[]).unwrap_err();
        assert_eq!(
            problems,
            vec![Problem::Conflict {
                namespace: Namespace::Type,
                name: "Command".into(),
                first: Owner::LeanType("A.Command".into()),
                second: Owner::LeanType("B.Command".into()),
            }]
        );
        assert!(problems[0].involves_lean_type());
    }

    #[test]
    fn a_lean_type_named_like_a_generated_type_or_unusable_is_rejected() {
        let ports = [
            port("a", "M.System"),
            port("b", "M.SignalInbox"),
            port("c", "M.Timer'"),
            port("d", "M.Self"),
        ];
        let problems = SystemNames::new(&example_children(), &ports, &[]).unwrap_err();
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems.iter().all(Problem::involves_lean_type));
    }

    #[test]
    fn a_module_named_next_and_an_edge_named_like_a_port_function() {
        let problems = SystemNames::new(
            &[child("signal", "next")],
            &[port("button", "M.Event")],
            &["port_button"],
        )
        .unwrap_err();
        let at: Vec<(Namespace, &str)> = problems
            .iter()
            .map(|p| match p {
                Problem::Conflict {
                    namespace, name, ..
                } => (*namespace, name.as_str()),
                Problem::NotARustName { .. } => panic!("{p:?}"),
            })
            .collect();
        assert_eq!(
            at,
            vec![
                (Namespace::Field, "next"),
                (Namespace::RouteFunction, "port_button")
            ]
        );
    }
}
