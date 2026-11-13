//! The names of a `composite/v0` module (`mod.rs` and `routes.rs`). Frozen with
//! the profile.

use std::collections::BTreeMap;

use super::{Claims, Namespace, Owner, Problem, rust_type_name, type_name};

/// Fields of `System` the profile names itself.
pub const FIELDS: &[&str] = &["next"];
/// Methods and associated items of `impl System` the profile names itself.
pub const METHODS: &[&str] = &[
    "CHILDREN",
    "deliver",
    "is_settled",
    "new",
    "run",
    "send",
    "step_once",
];
/// Types of the module the profile names itself, imports included.
pub const TYPES: &[&str] = &["Envelope", "System", "Unsettled", "VecDeque"];

/// A child of the composite, as its names are derived.
#[derive(Debug, Clone, Copy)]
pub struct Child<'a> {
    /// Component id.
    pub id: &'a str,
    /// Its `rust_module`.
    pub module: &'a str,
    /// True for a state machine, false for a nested composite.
    pub is_machine: bool,
}

/// Every name the generated module of one `composite/v0` uses, free of
/// conflicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemNames {
    children: BTreeMap<String, ChildNames>,
    fields: Vec<String>,
    methods: Vec<String>,
    types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChildNames {
    field: String,
    inbox: Option<String>,
    route_from: Option<String>,
    variant: String,
}

impl SystemNames {
    /// The names generated for `children` (with `edges`, the ids of the
    /// approved message edges inside the composite). `Err` lists every
    /// problem, not only the first.
    pub fn new(children: &[Child<'_>], edges: &[&str]) -> Result<SystemNames, Vec<Problem>> {
        let mut claims = Claims::default();
        claims.claim_fixed(Namespace::Field, FIELDS);
        claims.claim_fixed(Namespace::Method, METHODS);
        claims.claim_fixed(Namespace::Type, TYPES);
        let mut named = BTreeMap::new();
        for child in children {
            let owner = Owner::Component(child.id.to_string());
            let names = ChildNames::of(child);
            claims.claim(Namespace::Field, &names.field, &owner);
            claims.claim(Namespace::Method, &names.field, &owner);
            if let Some(inbox) = &names.inbox {
                claims.claim(Namespace::Field, inbox, &owner);
            }
            if let Some(route_from) = &names.route_from {
                claims.claim(Namespace::Method, route_from, &owner);
            }
            match rust_type_name(child.id) {
                Some(variant) => claims.claim(Namespace::Variant, &variant, &owner),
                None => claims.reject(Namespace::Variant, &names.variant, &owner),
            }
            named.insert(child.id.to_string(), names);
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
        claims.finish(SystemNames {
            children: named,
            fields,
            methods,
            types,
        })
    }

    fn child(&self, id: &str) -> &ChildNames {
        self.children
            .get(id)
            .unwrap_or_else(|| panic!("`{id}` is not a child of this composite"))
    }

    /// The field holding child `id`'s state (a machine) or nested system (a
    /// composite), and the read accessor of the same name.
    pub fn field(&self, id: &str) -> &str {
        &self.child(id).field
    }

    /// The field holding machine `id`'s mailbox.
    pub fn inbox(&self, id: &str) -> &str {
        self.child(id)
            .inbox
            .as_deref()
            .unwrap_or_else(|| panic!("`{id}` is not a machine"))
    }

    /// The method routing machine `id`'s outputs.
    pub fn route_from(&self, id: &str) -> &str {
        self.child(id)
            .route_from
            .as_deref()
            .unwrap_or_else(|| panic!("`{id}` is not a machine"))
    }

    /// The variant of `Envelope` addressed to child `id`.
    pub fn variant(&self, id: &str) -> &str {
        &self.child(id).variant
    }

    /// Every field of `System`, in order.
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Every method and associated item of `impl System`, in order.
    pub fn methods(&self) -> &[String] {
        &self.methods
    }

    /// Every type the module declares or imports, in order.
    pub fn types(&self) -> &[String] {
        &self.types
    }
}

impl ChildNames {
    fn of(child: &Child<'_>) -> ChildNames {
        let m = child.module;
        ChildNames {
            field: m.to_string(),
            inbox: child.is_machine.then(|| format!("{m}_inbox")),
            route_from: child.is_machine.then(|| format!("route_from_{m}")),
            variant: type_name(child.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine<'a>(id: &'a str, module: &'a str) -> Child<'a> {
        Child {
            id,
            module,
            is_machine: true,
        }
    }

    #[test]
    fn the_example_names() {
        let names = SystemNames::new(
            &[
                machine("request_latch", "request_latch"),
                machine("signal", "signal"),
            ],
            &["latch_to_signal"],
        )
        .unwrap();
        assert_eq!(names.field("signal"), "signal");
        assert_eq!(names.inbox("signal"), "signal_inbox");
        assert_eq!(names.route_from("signal"), "route_from_signal");
        assert_eq!(names.variant("request_latch"), "RequestLatch");
    }

    #[test]
    fn a_module_named_next_meets_the_scheduler_field() {
        let problems = SystemNames::new(&[machine("signal", "next")], &[]).unwrap_err();
        assert_eq!(
            problems,
            vec![Problem::Conflict {
                namespace: Namespace::Field,
                name: "next".into(),
                first: Owner::Profile,
                second: Owner::Component("signal".into()),
            }]
        );
    }

    #[test]
    fn derived_names_meet_each_other() {
        // `a`'s inbox field against a module named `a_inbox`; `route_from_a`
        // against a module of that name; `a_1` and `a1` give one variant.
        let problems = SystemNames::new(
            &[
                machine("a", "a"),
                machine("b", "a_inbox"),
                machine("c", "route_from_a"),
                machine("a_1", "d"),
                machine("a1", "e"),
            ],
            &[],
        )
        .unwrap_err();
        let names: Vec<(Namespace, &str)> = problems
            .iter()
            .map(|p| match p {
                Problem::Conflict {
                    namespace, name, ..
                } => (*namespace, name.as_str()),
                Problem::NotARustName { .. } => panic!("{p:?}"),
            })
            .collect();
        assert_eq!(
            names,
            vec![
                (Namespace::Field, "a_inbox"),
                (Namespace::Method, "route_from_a"),
                (Namespace::Variant, "A1"),
            ]
        );
    }

    #[test]
    fn a_module_named_like_a_method_meets_it() {
        let problems = SystemNames::new(&[machine("x", "run")], &[]).unwrap_err();
        assert!(matches!(
            &problems[..],
            [Problem::Conflict { namespace: Namespace::Method, name, .. }] if name == "run"
        ));
    }

    #[test]
    fn a_field_and_a_method_may_share_a_name() {
        assert!(SystemNames::new(&[machine("x", "x")], &[]).is_ok());
    }
}
