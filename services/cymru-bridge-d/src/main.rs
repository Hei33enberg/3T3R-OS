//! cymru-bridge-d entrypoint. Default: JSON-over-stdio **dev-bus** mock so the
//! cymru-agent radio gateway adapter can be developed against org.cymru.Radio
//! semantics with no hardware and no real D-Bus.
//!
//! Protocol: one JSON `Request` per line on stdin → one or more JSON `Event`
//! lines on stdout. Example:
//!   {"op":"subscribe","channel":""}
//!   {"op":"send","recipient":"broadcast","channel":"","payload":[104,105]}
//!
//! The real system-bus (zbus) backend is a follow-up (RFC 0002 C7).

use cymru_bridge_d::{Broker, Request};
use std::io::{BufRead, Write};

fn main() {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    tracing::info!("cymru-bridge-d {} — dev-bus (JSON/stdio) mock; zbus backend TODO", env!("CARGO_PKG_VERSION"));

    let mut broker = Broker::new(true);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            Ok(_) => continue,
            Err(_) => break,
        };
        let events = match serde_json::from_str::<Request>(&line) {
            Ok(req) => broker.handle(req),
            Err(e) => vec![cymru_bridge_d::Event::Error { message: format!("bad request: {e}") }],
        };
        for ev in events {
            if let Ok(s) = serde_json::to_string(&ev) {
                let _ = writeln!(stdout, "{s}");
            }
        }
        let _ = stdout.flush();
    }
}
