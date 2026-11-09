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

use l2r_model::{Component, Model, Port, profiles};

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

/// The two kinds of hop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopKind {
    /// A message edge: `Source → Option Target`.
    Edge,
    /// An input port: `Message → Target`, total.
    Port,
}

impl Flat {
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

impl<'a> Index<'a> {
    fn machine(&self, id: &str) -> Option<&'a Component> {
        self.model
            .component(id)
            .filter(|c| c.profile == profiles::TRANSITION_CORE_V1)
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
        let owner = self.owner_of_source(source);
        for edge in self.model.messages.iter().filter(|e| e.from == source) {
            let mut hops = prefix.to_vec();
            let edge_hop = |result: String| Hop {
                kind: HopKind::Edge,
                id: edge.id.clone(),
                owner: owner.clone(),
                translation: edge.translation.clone(),
                result,
            };
            if let Some(machine) = self.machine(&edge.to) {
                hops.push(edge_hop(self.input_of(machine)));
                out.push(FlatRoute {
                    hops,
                    target: Target::Leaf(machine.id.clone()),
                });
                continue;
            }
            let Some(port) = self.ports.get(edge.to.as_str()) else {
                continue;
            };
            hops.push(edge_hop(port.message.clone()));
            if !port.is_out() {
                if let Some(leaf) = self.descend(port, &mut hops) {
                    out.push(FlatRoute {
                        hops,
                        target: Target::Leaf(leaf),
                    });
                }
            } else if port.composite == self.root {
                out.push(FlatRoute {
                    hops,
                    target: Target::Sink(port.id.clone()),
                });
            } else {
                self.expand(&port.id, &hops, out);
            }
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
}

/// Computes the flat network of a structurally valid `v2` model (canonical
/// form: components, edges and ports sorted by identifier).
pub fn flatten(model: &Model) -> Result<Flat, Vec<FlatError>> {
    let root = model
        .components
        .iter()
        .find(|c| c.parent.is_none())
        .map(|c| c.id.as_str())
        .unwrap_or_default();
    let index = Index {
        model,
        ports: model.ports.iter().map(|p| (p.id.as_str(), p)).collect(),
        root,
    };
    let mut errors = Vec::new();
    let mut leaves = Vec::new();
    for machine in model
        .components
        .iter()
        .filter(|c| c.profile == profiles::TRANSITION_CORE_V1)
    {
        let mut routes = Vec::new();
        index.expand(&machine.id, &[], &mut routes);
        if routes.len() > MAX_ROUTES_PER_MACHINE {
            errors.push(FlatError::TooManyRoutes {
                machine: machine.id.clone(),
            });
        }
        if let Some(route) = routes
            .iter()
            .find(|r| r.target == Target::Leaf(machine.id.clone()))
        {
            errors.push(FlatError::SelfRoute {
                machine: machine.id.clone(),
                via: route.hops.iter().map(|h| h.id.clone()).collect(),
            });
        }
        leaves.push(Leaf {
            id: machine.id.clone(),
            capacity: machine.mailbox_capacity.unwrap_or(1),
            routes,
        });
    }
    let sinks = model
        .ports
        .iter()
        .filter(|p| p.composite == root && p.is_out())
        .map(|p| Sink {
            port: p.id.clone(),
            capacity: p.capacity.unwrap_or(1),
            message: p.message.clone(),
        })
        .collect();
    let inputs = model
        .ports
        .iter()
        .filter(|p| p.composite == root && !p.is_out())
        .filter_map(|p| {
            let mut hops = Vec::new();
            let target = index.descend(p, &mut hops)?;
            Some(FlatInput {
                port: p.id.clone(),
                message: p.message.clone(),
                hops,
                target,
            })
        })
        .collect();
    if errors.is_empty() {
        Ok(Flat {
            leaves,
            sinks,
            inputs,
        })
    } else {
        Err(errors)
    }
}
