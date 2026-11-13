//! The flat network of a hierarchical (`v2`) model.
//!
//! Hierarchy is structure: who contains whom, and which typed ports a
//! composite exposes. Behaviour is that of the flat network of the *machines*:
//! every output of a machine is carried, through the approved edges and ports
//! on its way, to the mailboxes of other machines or to an outbox of the root.
//! This module computes that network once, so that the generated Rust scheduler
//! and the derived Lean network are built from the same routes in the same
//! order.

use std::collections::BTreeMap;

use l2r_model::rust_names::composite_v2;
use l2r_model::{Component, Kind, MessageEdge, Model, Port, Schema};

/// The flat network of a `v2` model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flat {
    /// Every machine, in identifier order: the scheduler's children.
    pub leaves: Vec<Leaf>,
    /// The root's `out` ports, in identifier order: one bounded outbox each.
    pub sinks: Vec<Sink>,
    /// The root's `in` ports, in identifier order.
    pub inputs: Vec<FlatInput>,
}

/// A machine and where its outputs go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf {
    /// Component identifier.
    pub id: String,
    /// Mailbox capacity.
    pub capacity: u32,
    /// Every route an output takes, in routing order.
    pub routes: Vec<FlatRoute>,
}

/// An outbox of the root composite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sink {
    /// Port identifier.
    pub port: String,
    /// Outbox capacity.
    pub capacity: u32,
    /// Lean type of the messages.
    pub message: String,
}

/// An input port of the root and the machine it reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatInput {
    /// Port identifier.
    pub port: String,
    /// Lean type of the messages.
    pub message: String,
    /// The ports passed on the way down; every hop is total.
    pub hops: Vec<Hop>,
    /// The machine reached.
    pub target: String,
}

/// One route from a machine's output to a receiver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatRoute {
    /// The edges and ports passed, in order. The first is always an edge.
    pub hops: Vec<Hop>,
    /// The receiver.
    pub target: Target,
}

/// Where a route ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The mailbox of a machine.
    Leaf(String),
    /// An outbox of the root composite.
    Sink(String),
}

/// One translation on a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    /// Whether the hop is a message edge (partial) or an input port (total).
    pub kind: HopKind,
    /// Edge or port identifier.
    pub id: String,
    /// The composite whose `routes.rs` implements the hop.
    pub owner: String,
    /// The Lean translation.
    pub translation: String,
    /// Lean type of the hop's result.
    pub result: String,
}

impl Hop {
    /// The `routes.rs` function implementing this hop.
    pub fn function(&self) -> String {
        match self.kind {
            HopKind::Edge => composite_v2::Edge(&self.id).function(),
            HopKind::Port => composite_v2::Port(&self.id).function(),
        }
    }
}

/// The two kinds of hop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopKind {
    /// A message edge: `Source → Option Target`.
    Edge,
    /// An input port: `Message → Target`, total.
    Port,
}

impl Flat {
    /// The hops whose translation composite `composite`'s `routes.rs`
    /// implements, one per function, edges first, each kind by id.
    pub fn hops_owned_by(&self, composite: &str) -> Vec<&Hop> {
        let mut seen = std::collections::BTreeSet::new();
        let mut owned: Vec<&Hop> = self
            .leaves
            .iter()
            .flat_map(|l| l.routes.iter().flat_map(|r| r.hops.iter()))
            .chain(self.inputs.iter().flat_map(|i| i.hops.iter()))
            .filter(|h| h.owner == composite && seen.insert(h.function()))
            .collect();
        owned.sort_by_key(|h| (h.kind == HopKind::Port, h.id.clone()));
        owned
    }

    /// Scheduler index of a machine.
    pub fn leaf_index(&self, id: &str) -> Option<usize> {
        self.leaves.iter().position(|l| l.id == id)
    }

    /// Network index of an outbox: after every machine.
    pub fn sink_index(&self, port: &str) -> Option<usize> {
        self.sinks
            .iter()
            .position(|s| s.port == port)
            .map(|i| self.leaves.len() + i)
    }

    /// Network index of a route's receiver.
    pub fn target_index(&self, target: &Target) -> Option<usize> {
        match target {
            Target::Leaf(id) => self.leaf_index(id),
            Target::Sink(port) => self.sink_index(port),
        }
    }
}

/// Upper bound on the routes of one machine; a model beyond it is rejected
/// rather than generating an unreadable scheduler.
pub const MAX_ROUTES_PER_MACHINE: usize = 32;

/// Why a structurally valid model has no flat network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatError {
    /// A machine's output can come back to its own mailbox.
    SelfRoute {
        /// The machine.
        machine: String,
        /// The edges and ports of the offending route.
        via: Vec<String>,
    },
    /// A machine has more than [`MAX_ROUTES_PER_MACHINE`] routes.
    TooManyRoutes {
        /// The machine.
        machine: String,
    },
}

struct Index<'a> {
    model: &'a Model,
    ports: BTreeMap<&'a str, &'a Port>,
    root: &'a str,
}

fn is_machine(component: &Component) -> bool {
    component
        .parsed_profile()
        .is_some_and(|p| Schema::V2.admits(p) && p.kind() == Kind::Machine)
}

impl<'a> Index<'a> {
    fn new(model: &'a Model) -> Index<'a> {
        let root = model
            .components
            .iter()
            .find(|c| c.parent.is_none())
            .map(|c| c.id.as_str())
            .unwrap_or_default();
        Index {
            model,
            ports: model.ports.iter().map(|p| (p.id.as_str(), p)).collect(),
            root,
        }
    }

    fn machine(&self, id: &str) -> Option<&'a Component> {
        self.model.component(id).filter(|c| is_machine(c))
    }

    fn input_of(&self, machine: &Component) -> String {
        machine
            .behavior
            .as_ref()
            .map(|b| b.input.clone())
            .unwrap_or_default()
    }

    /// Follows an input port down to the machine it reaches.
    fn descend(&self, port: &'a Port, hops: &mut Vec<Hop>) -> Option<String> {
        let mut port = port;
        // Each step enters a child composite, so the walk is bounded by the depth.
        for _ in 0..=self.model.components.len() {
            let target = port.target();
            let hop = |result: String| Hop {
                kind: HopKind::Port,
                id: port.id.clone(),
                owner: port.composite.clone(),
                translation: port.translation_name().to_string(),
                result,
            };
            if let Some(machine) = self.machine(target) {
                hops.push(hop(self.input_of(machine)));
                return Some(machine.id.clone());
            }
            let next = self.ports.get(target)?;
            hops.push(hop(next.message.clone()));
            port = next;
        }
        None
    }

    /// Every route from `source` (a machine or an `out` port of a non-root
    /// composite), given the hops already passed.
    fn expand(&self, source: &str, prefix: &[Hop], out: &mut Vec<FlatRoute>) {
        if out.len() > MAX_ROUTES_PER_MACHINE {
            return;
        }
        for edge in self.model.messages.iter().filter(|e| e.from == source) {
            self.follow(edge, prefix, out);
        }
    }

    /// Every route that passes `prefix` and then `edge`.
    fn follow(&self, edge: &MessageEdge, prefix: &[Hop], out: &mut Vec<FlatRoute>) {
        let mut hops = prefix.to_vec();
        let edge_hop = |result: String| Hop {
            kind: HopKind::Edge,
            id: edge.id.clone(),
            owner: self.owner_of_source(&edge.from),
            translation: edge.translation.clone(),
            result,
        };
        if let Some(machine) = self.machine(&edge.to) {
            hops.push(edge_hop(self.input_of(machine)));
            out.push(FlatRoute {
                hops,
                target: Target::Leaf(machine.id.clone()),
            });
            return;
        }
        let Some(port) = self.ports.get(edge.to.as_str()) else {
            return;
        };
        hops.push(edge_hop(port.message.clone()));
        self.continue_from(port, hops, out);
    }

    /// Every route whose `hops` have brought it to `port`: an input port leads
    /// down to one machine, an `out` port of the root is an outbox, and any
    /// other `out` port hands over to the edges of the composite above.
    fn continue_from(&self, port: &'a Port, mut hops: Vec<Hop>, out: &mut Vec<FlatRoute>) {
        if !port.is_out() {
            let leaf = self.descend(port, &mut hops);
            out.extend(leaf.map(|leaf| FlatRoute {
                hops,
                target: Target::Leaf(leaf),
            }));
        } else if port.composite == self.root {
            out.push(FlatRoute {
                hops,
                target: Target::Sink(port.id.clone()),
            });
        } else {
            self.expand(&port.id, &hops, out);
        }
    }

    /// The composite in which the edges leaving `source` live.
    fn owner_of_source(&self, source: &str) -> String {
        let component = match self.ports.get(source) {
            Some(port) => self.model.component(&port.composite),
            None => self.model.component(source),
        };
        component.and_then(|c| c.parent.clone()).unwrap_or_default()
    }

    /// A machine with every route its outputs take.
    fn leaf(&self, machine: &Component) -> Leaf {
        let mut routes = Vec::new();
        self.expand(&machine.id, &[], &mut routes);
        Leaf {
            id: machine.id.clone(),
            capacity: machine.mailbox_capacity.unwrap_or(1),
            routes,
        }
    }

    /// The outboxes: the root's `out` ports.
    fn sinks(&self) -> Vec<Sink> {
        let out_ports = self.root_ports().filter(|p| p.is_out());
        out_ports
            .map(|p| Sink {
                port: p.id.clone(),
                capacity: p.capacity.unwrap_or(1),
                message: p.message.clone(),
            })
            .collect()
    }

    /// The root's input ports, each followed down to the machine it reaches.
    fn inputs(&self) -> Vec<FlatInput> {
        let in_ports = self.root_ports().filter(|p| !p.is_out());
        in_ports
            .filter_map(|p| {
                let mut hops = Vec::new();
                let target = self.descend(p, &mut hops)?;
                Some(FlatInput {
                    port: p.id.clone(),
                    message: p.message.clone(),
                    hops,
                    target,
                })
            })
            .collect()
    }

    fn root_ports(&self) -> impl Iterator<Item = &'a Port> {
        let root = self.root;
        self.model.ports.iter().filter(move |p| p.composite == root)
    }
}

impl Leaf {
    /// Why the machine's routes cannot be scheduled: there are too many, or
    /// one leads back to the machine itself.
    fn route_errors(&self) -> Vec<FlatError> {
        let mut errors = Vec::new();
        if self.routes.len() > MAX_ROUTES_PER_MACHINE {
            errors.push(FlatError::TooManyRoutes {
                machine: self.id.clone(),
            });
        }
        let to_itself = Target::Leaf(self.id.clone());
        if let Some(route) = self.routes.iter().find(|r| r.target == to_itself) {
            errors.push(FlatError::SelfRoute {
                machine: self.id.clone(),
                via: route.hops.iter().map(|h| h.id.clone()).collect(),
            });
        }
        errors
    }
}

/// Computes the flat network of a structurally valid `v2` model (canonical
/// form: components, edges and ports sorted by identifier).
pub fn flatten(model: &Model) -> Result<Flat, Vec<FlatError>> {
    let index = Index::new(model);
    let machines = model.components.iter().filter(|c| is_machine(c));
    let leaves: Vec<Leaf> = machines.map(|machine| index.leaf(machine)).collect();
    let errors: Vec<FlatError> = leaves.iter().flat_map(Leaf::route_errors).collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Flat {
        leaves,
        sinks: index.sinks(),
        inputs: index.inputs(),
    })
}
