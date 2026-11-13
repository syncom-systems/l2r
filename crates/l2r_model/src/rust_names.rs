//! The Rust names generated code uses, one owner per profile version.
//!
//! What a composite's generated module calls its fields, methods, types and
//! route functions is part of the profile's published interface: implementers
//! write code against it, work packets and conformance harnesses name it. It is
//! decided here once. The planner emits these names, and validation checks with
//! the same constructors that no two things the architect named apart meet as
//! one name, before any plan can be approved.
//!
//! Each frozen profile version has its own sub-module, so that a later version
//! can change its scheme without moving a byte of an earlier one. A name set
//! ([`composite_v0::SystemNames`], [`composite_v1::SystemNames`]) exists only
//! when it is free of conflicts.

use std::collections::BTreeMap;
use std::fmt;

pub mod composite_v0;
pub mod composite_v1;

/// `request_latch` → `RequestLatch`: each `_`-separated part with its first
/// character in upper case. Every profile version's output depends on this
/// conversion, so it never changes; see [`rust_type_name`] for when the result
/// is a usable type name.
pub fn type_name(identifier: &str) -> String {
    identifier
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// [`type_name`] of `identifier`, when that is a Rust type name: ASCII letters,
/// digits and `_` only, starting with an upper-case letter, and not `Self`.
pub fn rust_type_name(identifier: &str) -> Option<String> {
    let usable = identifier
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_');
    let name = type_name(identifier);
    let valid = usable && name.starts_with(|c: char| c.is_ascii_uppercase()) && name != "Self";
    valid.then_some(name)
}

/// The last `.`-separated segment of a Lean name.
fn lean_last_segment(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// Where in the generated code a name lives. Two things meet only within one
/// namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Namespace {
    /// Fields of the generated `System`.
    Field,
    /// Methods and associated items of `impl System`.
    Method,
    /// Types declared in the composite's module.
    Type,
    /// Variants of one generated enum.
    Variant,
    /// Functions of the composite's `routes.rs`.
    RouteFunction,
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Namespace::Field => "the `System` field",
            Namespace::Method => "the `System` method",
            Namespace::Type => "the type",
            Namespace::Variant => "the enum variant",
            Namespace::RouteFunction => "the `routes.rs` function",
        })
    }
}

/// What a generated name was derived from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    /// A child component, by id.
    Component(String),
    /// A port, by id.
    Port(String),
    /// A message edge, by id.
    Edge(String),
    /// A Lean type a port carries, by qualified name.
    LeanType(String),
    /// A name the profile itself generates.
    Profile,
}

impl fmt::Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Owner::Component(id) => write!(f, "component `{id}`"),
            Owner::Port(id) => write!(f, "port `{id}`"),
            Owner::Edge(id) => write!(f, "message `{id}`"),
            Owner::LeanType(name) => write!(f, "Lean type `{name}`"),
            Owner::Profile => f.write_str("the profile"),
        }
    }
}

/// Why a set of generated names cannot exist.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Problem {
    /// Two different owners give the same name in one namespace.
    Conflict {
        /// Where the names meet.
        namespace: Namespace,
        /// The name.
        name: String,
        /// The owner that named it first.
        first: Owner,
        /// The owner that names it again.
        second: Owner,
    },
    /// An owner gives a name that is not a Rust name of its kind.
    NotARustName {
        /// Where the name would live.
        namespace: Namespace,
        /// The name.
        name: String,
        /// Its owner.
        owner: Owner,
    },
}

impl Problem {
    /// The owner the problem is reported against: the later one of a conflict.
    pub fn subject(&self) -> &Owner {
        match self {
            Problem::Conflict { second, .. } => second,
            Problem::NotARustName { owner, .. } => owner,
        }
    }

    /// True when a Lean type is involved, rather than only identifiers the
    /// architect chose.
    pub fn involves_lean_type(&self) -> bool {
        match self {
            Problem::Conflict { first, second, .. } => {
                matches!(first, Owner::LeanType(_)) || matches!(second, Owner::LeanType(_))
            }
            Problem::NotARustName { owner, .. } => matches!(owner, Owner::LeanType(_)),
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Conflict {
                namespace,
                name,
                first,
                ..
            } => write!(
                f,
                "generates {namespace} `{name}`, which {first} also generates"
            ),
            Problem::NotARustName {
                namespace, name, ..
            } => write!(
                f,
                "generates {namespace} `{name}`, which is not a usable Rust name"
            ),
        }
    }
}

/// The names of one generated module, by namespace, while they are collected.
#[derive(Default)]
struct Claims {
    taken: BTreeMap<(Namespace, String), Owner>,
    problems: Vec<Problem>,
}

impl Claims {
    /// Records that `owner` gives `name` in `namespace`. The same owner may
    /// claim a name twice (a field and its accessor are one decision).
    fn claim(&mut self, namespace: Namespace, name: &str, owner: &Owner) {
        let key = (namespace, name.to_string());
        match self.taken.get(&key) {
            Some(first) if first != owner => self.problems.push(Problem::Conflict {
                namespace,
                name: name.to_string(),
                first: first.clone(),
                second: owner.clone(),
            }),
            Some(_) => {}
            None => {
                self.taken.insert(key, owner.clone());
            }
        }
    }

    fn claim_fixed(&mut self, namespace: Namespace, names: &[&str]) {
        for name in names {
            self.claim(namespace, name, &Owner::Profile);
        }
    }

    /// Claims the route function of each message edge: the edge's own id.
    fn claim_edges(&mut self, edges: &[&str]) {
        for edge in edges {
            self.claim(
                Namespace::RouteFunction,
                edge,
                &Owner::Edge(edge.to_string()),
            );
        }
    }

    fn reject(&mut self, namespace: Namespace, name: &str, owner: &Owner) {
        self.problems.push(Problem::NotARustName {
            namespace,
            name: name.to_string(),
            owner: owner.clone(),
        });
    }

    fn finish<T>(mut self, names: T) -> Result<T, Vec<Problem>> {
        if self.problems.is_empty() {
            Ok(names)
        } else {
            self.problems.sort();
            self.problems.dedup();
            Err(self.problems)
        }
    }

    /// Every claimed name of `namespace`, in order.
    fn names(&self, namespace: Namespace) -> Vec<String> {
        self.taken
            .keys()
            .filter(|(n, _)| *n == namespace)
            .map(|(_, name)| name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_names() {
        assert_eq!(type_name("request_latch"), "RequestLatch");
        assert_eq!(type_name("signal"), "Signal");
        assert_eq!(type_name("a_1"), "A1");
        assert_eq!(type_name("Timer'"), "Timer'");
    }

    #[test]
    fn usable_type_names() {
        assert_eq!(
            rust_type_name("button_event").as_deref(),
            Some("ButtonEvent")
        );
        assert_eq!(rust_type_name("TimerEvent").as_deref(), Some("TimerEvent"));
        for unusable in ["Timer'", "ok?", "self", "Self", "1a", "_", ""] {
            assert_eq!(rust_type_name(unusable), None, "{unusable:?}");
        }
    }

    #[test]
    fn a_name_claimed_twice_by_one_owner_is_no_conflict() {
        let mut claims = Claims::default();
        let a = Owner::Component("a".into());
        claims.claim(Namespace::Method, "a", &a);
        claims.claim(Namespace::Method, "a", &a);
        claims.claim(Namespace::Field, "a", &Owner::Component("b".into()));
        assert!(claims.finish(()).is_ok());
    }
}
