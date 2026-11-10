//! The route functions of the `System`: for every route of every machine, the
//! translations a message passes on that route, composed.

use l2r_validate::flat::{FlatRoute, Hop, HopKind, Leaf, Target};

use super::{receiver, via};
use crate::text;
use crate::v2::Gen;

/// Every route function, machine by machine, in routing order.
pub(super) fn route_functions(generator: &Gen<'_>) -> String {
    generator
        .flat
        .leaves
        .iter()
        .flat_map(|leaf| {
            leaf.routes
                .iter()
                .enumerate()
                .map(move |(index, route)| route_function(generator, leaf, index, route))
        })
        .collect()
}

/// The function composing the translations of one route.
fn route_function(generator: &Gen<'_>, leaf: &Leaf, index: usize, route: &FlatRoute) -> String {
    let ty = text::type_name(&leaf.id);
    let body = route_expr(generator, &route.hops, "output", "    ");
    let allow = if ends_in_a_port(route) {
        "#[allow(clippy::manual_map)]\n"
    } else {
        ""
    };
    format!(
        "/// Route {index} of `{id}`: to {target} through {via}.\n\
         {allow}fn {id}_route_{index}(output: {ty}Output) -> Option<{element}> {{\n    {body}\n}}\n\n",
        id = leaf.id,
        target = receiver(&route.target),
        via = via(&route.hops),
        element = element(&route.target),
    )
}

/// The type of the messages a route delivers.
fn element(target: &Target) -> String {
    match target {
        Target::Leaf(id) => format!("{}Input", text::type_name(id)),
        Target::Sink(port) => format!("{}Message", text::type_name(port)),
    }
}

/// A route whose last hop is an input port ends in `Some(..)` of a total
/// translation. The explicit match around it is the shape the Lean model has;
/// clippy would rewrite it as a `map`.
fn ends_in_a_port(route: &FlatRoute) -> bool {
    route
        .hops
        .last()
        .is_some_and(|hop| hop.kind == HopKind::Port)
}

/// The expression that carries `value` through `hops`. An input port is total,
/// so it wraps the value; a message edge is matched on, and `None` ends the route.
fn route_expr(generator: &Gen<'_>, hops: &[Hop], value: &str, indent: &str) -> String {
    let Some((hop, rest)) = hops.split_first() else {
        return value.to_string();
    };
    let call = format!("{}({value})", generator.hop_function(hop));
    match (hop.kind, rest.is_empty()) {
        (HopKind::Edge, true) => call,
        (HopKind::Port, true) => format!("Some({call})"),
        (HopKind::Port, false) => route_expr(generator, rest, &call, indent),
        (HopKind::Edge, false) => {
            let inner = format!("{indent}    ");
            format!(
                "match {call} {{\n{inner}Some(message) => {},\n{inner}None => None,\n{indent}}}",
                route_expr(generator, rest, "message", &inner)
            )
        }
    }
}
