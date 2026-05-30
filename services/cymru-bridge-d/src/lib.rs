//! cymru-bridge-d core — the broker contract for `org.cymru.Radio` / `org.cymru.Robot`
//! (RFC 0002). The real transport is the system D-Bus; this module is bus-agnostic
//! so it can run as a JSON-over-stdio **dev-bus** mock (see `main.rs`) that app
//! adapters develop against with zero hardware.

use serde::{Deserialize, Serialize};

pub mod robot;
pub use robot::{RobotBroker, RobotVerdict};

/// A request an app sends to the bridge (mirrors org.cymru.Radio methods).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Subscribe to a channel (""=all frames addressed to this device). Returns a sub id.
    Subscribe {
        channel: String,
    },
    Unsubscribe {
        sub_id: u64,
    },
    /// Direct send. `recipient` = hex device-id or "broadcast".
    Send {
        recipient: String,
        channel: String,
        payload: Vec<u8>,
    },
    /// Carrier availability query.
    GetCarriers,
}

/// An event/response the bridge emits to apps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Subscribed {
        sub_id: u64,
    },
    Unsubscribed {
        sub_id: u64,
    },
    /// Mirrors the `MessageReceived` signal.
    MessageReceived {
        sub_id: u64,
        sender: String,
        channel: String,
        payload: Vec<u8>,
    },
    Carriers {
        lora_868: bool,
        lora_915: bool,
        hf_codec2: bool,
    },
    Error {
        message: String,
    },
}

/// Bus-agnostic broker. In dev (mock) mode `send` loops messages back to matching
/// subscribers so adapters can be exercised end-to-end without a peer radio.
#[derive(Debug, Default)]
pub struct Broker {
    subs: Vec<(u64, String)>,
    next_sub: u64,
    /// Loopback delivery for dev mode (real mode routes to cymru-radio-d instead).
    pub loopback: bool,
}

impl Broker {
    pub fn new(loopback: bool) -> Self {
        Broker {
            subs: Vec::new(),
            next_sub: 1,
            loopback,
        }
    }

    pub fn handle(&mut self, req: Request) -> Vec<Event> {
        match req {
            Request::Subscribe { channel } => {
                let id = self.next_sub;
                self.next_sub += 1;
                self.subs.push((id, channel));
                vec![Event::Subscribed { sub_id: id }]
            }
            Request::Unsubscribe { sub_id } => {
                self.subs.retain(|(id, _)| *id != sub_id);
                vec![Event::Unsubscribed { sub_id }]
            }
            Request::GetCarriers => vec![Event::Carriers {
                lora_868: true,
                lora_915: false,
                hf_codec2: false,
            }],
            Request::Send {
                recipient,
                channel,
                payload,
            } => {
                if !self.loopback {
                    // Real mode would hand off to cymru-radio-d here.
                    return vec![];
                }
                // Dev loopback: deliver to subscribers whose channel matches ("" = wildcard).
                let _ = recipient;
                self.subs
                    .iter()
                    .filter(|(_, c)| c.is_empty() || *c == channel)
                    .map(|(id, _)| Event::MessageReceived {
                        sub_id: *id,
                        sender: "dev-loopback".into(),
                        channel: channel.clone(),
                        payload: payload.clone(),
                    })
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_then_loopback_delivers() {
        let mut b = Broker::new(true);
        let sub = b.handle(Request::Subscribe {
            channel: String::new(),
        });
        let sub_id = match sub.as_slice() {
            [Event::Subscribed { sub_id }] => *sub_id,
            _ => panic!("no sub id"),
        };
        let ev = b.handle(Request::Send {
            recipient: "broadcast".into(),
            channel: "mesh".into(),
            payload: vec![1, 2, 3],
        });
        assert_eq!(
            ev,
            vec![Event::MessageReceived {
                sub_id,
                sender: "dev-loopback".into(),
                channel: "mesh".into(),
                payload: vec![1, 2, 3],
            }]
        );
    }

    #[test]
    fn json_request_parses() {
        let r: Request = serde_json::from_str(
            r#"{"op":"send","recipient":"broadcast","channel":"","payload":[7,8]}"#,
        )
        .unwrap();
        matches!(r, Request::Send { .. });
    }

    #[test]
    fn real_mode_does_not_loopback() {
        let mut b = Broker::new(false);
        b.handle(Request::Subscribe {
            channel: String::new(),
        });
        let ev = b.handle(Request::Send {
            recipient: "x".into(),
            channel: "".into(),
            payload: vec![1],
        });
        assert!(ev.is_empty());
    }
}
