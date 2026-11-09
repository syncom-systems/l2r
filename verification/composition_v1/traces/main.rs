//! Records what the generated `composite/v1` scheduler of the bounded crossing
//! example does on every operation sequence up to a fixed length, for comparison
//! with the Lean scheduler model (`L2R.Async1.RoundRobin`) by `Conformance.lean`.
//!
//! This is bounded testing (kind 5 evidence), not proof. Unlike the v0 lane it
//! observes the **contents of both mailboxes** after every operation, so
//! capacity, rejection and blocking behaviour are compared, not only states.
//!
//! Output: one line per sequence, `<ops> <observations>`. Ops: `p` offers a
//! button press to the latch's port, `t` a timer expiry to the signal's port,
//! `s` calls `step_once`. The observation after each op is
//! `<result>/<latch>/<signal>/<latch mailbox>/<signal mailbox>`: `<result>` is
//! `ok`/`rej` for a send and `D`/`S`/`B` (delivered, settled, blocked) for
//! `step_once`; a mailbox is its variant names joined by `,`, or `-` if empty.

use crossing_v1_scaffold::crossing::{ButtonEvent, Progress, System, TimerEvent};

const OPS: [char; 3] = ['p', 't', 's'];

fn latch_mailbox(system: &System) -> String {
    let inbox = system.request_latch_inbox();
    let items: Vec<String> = (0..inbox.len())
        .map(|i| format!("{:?}", inbox.get(i).expect("within length")))
        .collect();
    if items.is_empty() {
        "-".into()
    } else {
        items.join(",")
    }
}

fn signal_mailbox(system: &System) -> String {
    let inbox = system.signal_inbox();
    let items: Vec<String> = (0..inbox.len())
        .map(|i| format!("{:?}", inbox.get(i).expect("within length")))
        .collect();
    if items.is_empty() {
        "-".into()
    } else {
        items.join(",")
    }
}

fn apply(system: &mut System, op: char) -> String {
    let result = match op {
        'p' => match system.send_button(ButtonEvent::Pressed) {
            Ok(()) => "ok",
            Err(_) => "rej",
        },
        't' => match system.send_timer(TimerEvent::Expired) {
            Ok(()) => "ok",
            Err(_) => "rej",
        },
        _ => match system.step_once() {
            Progress::Delivered => "D",
            Progress::Settled => "S",
            Progress::Blocked => "B",
        },
    };
    format!(
        "{result}/{:?}/{:?}/{}/{}",
        system.request_latch(),
        system.signal(),
        latch_mailbox(system),
        signal_mailbox(system)
    )
}

fn main() {
    let length: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .expect("usage: scheduler_traces <length>");
    for n in 0..OPS.len().pow(length) {
        let mut code = n;
        let ops: String = (0..length)
            .map(|_| {
                let op = OPS[code % OPS.len()];
                code /= OPS.len();
                op
            })
            .collect();
        let mut system = System::new();
        let observed: Vec<String> = ops.chars().map(|op| apply(&mut system, op)).collect();
        println!("{ops} {}", observed.join(" "));
    }
}
