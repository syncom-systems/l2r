//! Records what the generated `composite/v0` scheduler of the crossing example
//! does on every operation sequence up to a fixed length, for comparison with
//! the Lean scheduler model (`L2R.Async.RoundRobin`) by `Conformance.lean`.
//!
//! This is bounded testing (kind 5 evidence), not proof. It exercises the one
//! link of the composition argument that is reviewed rather than proved: that
//! the Lean scheduler model describes the generated Rust `System`.
//!
//! Output: one line per sequence, `<ops> <observations>`. Ops: `p` sends
//! `ButtonPressed` and `a` `Acknowledged` to the latch, `r` sends
//! `CrossingRequested` and `t` `TimerExpired` to the signal, `s` calls
//! `step_once`. After each op the observation is `<step>/<latch>/<signal>`,
//! where `<step>` is `1`/`0` for `step_once`'s result and `-` for a send, and
//! the states are the Rust variant names.

use crossing_scaffold::crossing::{Envelope, System, request_latch, signal};

const OPS: [char; 5] = ['p', 'a', 'r', 't', 's'];

fn apply(system: &mut System, op: char) -> String {
    let step = match op {
        'p' => {
            system.send(Envelope::RequestLatch(request_latch::Input::ButtonPressed));
            "-"
        }
        'a' => {
            system.send(Envelope::RequestLatch(request_latch::Input::Acknowledged));
            "-"
        }
        'r' => {
            system.send(Envelope::Signal(signal::Input::CrossingRequested));
            "-"
        }
        't' => {
            system.send(Envelope::Signal(signal::Input::TimerExpired));
            "-"
        }
        _ => {
            if system.step_once() {
                "1"
            } else {
                "0"
            }
        }
    };
    format!("{step}/{:?}/{:?}", system.request_latch(), system.signal())
}

fn main() {
    let length: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .expect("usage: scheduler_traces <length>");
    // Every sequence of exactly `length` ops; prefixes are observed along the way.
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
