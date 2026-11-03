//! `composite/v1` on shapes other than the crossing: three children with
//! different capacities (including 1, where the mailbox shifts nothing), a ring
//! of edges, and two ports that share a message type. The generated crate of this
//! exact model was also compiled with `rustc -D warnings` when it was written.
//! Output of `composite/v1` for the crossing example is frozen below.

use l2r_plan::{GeneratorIdentity, Plan, plan};
use l2r_testing::{example_v1_inventory, example_v1_model};
use l2r_validate::validate;

const RING: &str = include_str!("fixtures/ring.toml");
const RING_INVENTORY: &str = include_str!("fixtures/ring-inventory.json");

fn ring_plan() -> Plan {
    let model = l2r_model::load::parse_model(RING).expect("ring parses");
    let inventory = l2r_model::load::parse_inventory(RING_INVENTORY).expect("inventory parses");
    plan(
        &validate(&model, &inventory).expect("ring is valid"),
        &GeneratorIdentity::current(),
    )
    .expect("plans")
}

fn contract(plan: &Plan) -> &str {
    &plan
        .artifact("src/ring/mod.rs")
        .expect("composite contract is planned")
        .content
}

#[test]
fn capacities_become_constants_and_array_sizes() {
    let plan = ring_plan();
    let text = contract(&plan);
    for (name, cap) in [("A", 1), ("B", 3), ("C", 2)] {
        assert!(
            text.contains(&format!("pub const {name}_CAPACITY: usize = {cap};")),
            "{name}"
        );
        assert!(
            text.contains(&format!(
                "slots: [Option<{}::Input>; {cap}]",
                name.to_lowercase()
            )),
            "{name}"
        );
    }
}

#[test]
fn pop_shifts_exactly_as_many_slots_as_the_capacity_needs() {
    let plan = ring_plan();
    let text = contract(&plan);
    // capacity 1: nothing to shift, the single slot is cleared.
    assert!(text.contains("let first = self.slots[0];\n        self.slots[0] = None;"));
    // capacity 3: two shifts, then the last slot is cleared.
    assert!(text.contains(
        "self.slots[0] = self.slots[1];\n        self.slots[1] = self.slots[2];\n        self.slots[2] = None;"
    ));
}

#[test]
fn the_scheduler_tries_every_child_once_starting_at_the_cursor() {
    let plan = ring_plan();
    let text = contract(&plan);
    assert!(text.contains("let child = (self.next) % Self::CHILDREN;"));
    assert!(text.contains("let child = (self.next + 1) % Self::CHILDREN;"));
    assert!(text.contains("let child = (self.next + 2) % Self::CHILDREN;"));
    assert!(!text.contains("self.next + 3"));
    assert_eq!(text.matches("match self.attempt(child)").count(), 3);
    assert!(text.contains("const CHILDREN: usize = 3;"));
}

#[test]
fn ports_sharing_a_message_type_share_one_enum_and_each_gets_a_send() {
    let plan = ring_plan();
    let text = contract(&plan);
    assert_eq!(text.matches("pub enum Ping {").count(), 1);
    assert!(text.contains("pub fn send_first(&mut self, message: Ping)"));
    assert!(text.contains("pub fn send_second(&mut self, message: Ping)"));
    assert!(text.contains("routes::port_first(message)"));
    let routes = &plan.artifact("src/ring/routes.rs").unwrap().content;
    assert!(routes.contains("pub(super) fn port_first(message: Ping) -> a::Input"));
    assert!(routes.contains("pub(super) fn port_second(message: Ping) -> c::Input"));
}

#[test]
fn a_full_receiver_blocks_the_sender_and_the_check_precedes_every_change() {
    let plan = ring_plan();
    let text = contract(&plan);
    // For child `a` (edge a_to_b): the room check comes before the pop.
    let attempt = text
        .split("fn attempt_a(&mut self) -> Attempt {")
        .nth(1)
        .and_then(|rest| rest.split("fn attempt_b").next())
        .expect("attempt_a is generated");
    let check = attempt.find("return Attempt::Blocked;").expect("blocks");
    let pop = attempt.find("self.a_inbox.pop()").expect("pops");
    assert!(check < pop, "the room check must precede any mutation");
}

/// Approved generation behavior must never change silently. As with
/// `v0_profile_output_is_frozen`, a change to what `composite/v1` generates
/// requires a new profile version.
#[test]
fn v1_profile_output_is_frozen() {
    let plan = plan(
        &validate(&example_v1_model(), &example_v1_inventory()).expect("valid"),
        &GeneratorIdentity::current(),
    )
    .expect("plans");
    let mut by_profile: std::collections::BTreeMap<&str, Vec<(&str, &str)>> = Default::default();
    for artifact in &plan.artifacts {
        by_profile
            .entry(artifact.profile.as_str())
            .or_default()
            .push((artifact.path.as_str(), artifact.content.as_str()));
    }
    let digests: Vec<(&str, String)> = by_profile
        .iter()
        .map(|(profile, files)| (*profile, l2r_model::digest::digest_of(files)))
        .collect();
    let expected: [(&str, &str); 3] = [
        (
            "composite/v1",
            "sha256:2048c1c61f6fb5dc1facdae79cc88825e845a50da85650fe7249901af7ba985b",
        ),
        (
            "crate-root/v0",
            "sha256:49c7715bc98ec8c72a186880ef0551234f8c7d48512c37efa1a63b4d11de560a",
        ),
        (
            "transition-core/v0",
            "sha256:2c91b508987d69ffd9a0d269f6e2782b1c9f3f16b946192152ec999ad5399b04",
        ),
    ];
    let expected: Vec<(&str, String)> = expected.iter().map(|(p, d)| (*p, d.to_string())).collect();
    assert_eq!(digests, expected);
}
