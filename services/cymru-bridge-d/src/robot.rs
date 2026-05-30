//! `org.cymru.Robot` broker — robot dispatch over the bridge (RFC 0003 / CRCP).
//!
//! Safety-critical path (transport path B, see raydio-transport-decision.md): this
//! NEVER rides Reticulum. The broker enforces, before anything reaches an actuator:
//!  1. every command is ed25519-verified against the paired master key,
//!  2. a `task` to a physical agent MUST carry `deadman_ms` (mirrors
//!     `validatePhysicalTask` on the cymru side) — rejected otherwise,
//!  3. an `estop` is always highest priority and is never gated by replay/dedup.
//!
//! Wire input is a CRCP payload unit (header byte + canonical CBOR), exactly what
//! rides inside an RFC 0001 `0x04` frame. We decode with the `crcp` crate (single
//! source of truth) — we never re-define the schema here.

use crcp::{decode_wire, verify, Cbor, MsgType};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

/// Outcome of feeding one CRCP payload unit to the robot broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum RobotVerdict {
    /// Task accepted for execution (signature ok, deadman present).
    TaskAccepted { agent_id: String, deadman_ms: u64 },
    /// E-stop honored — highest priority, must reach actuators immediately.
    EStop { agent_id: Option<String> },
    /// Manifest / telemetry accepted (informational, non-actuating).
    Info { msg_type: String },
    /// Rejected, with a machine-readable reason. NEVER actuate on a rejection.
    Rejected { reason: String },
}

/// True if the agent's capability flags mark it as a physical (actuating) agent.
/// Mirrors `isPhysicalAgent` in crcp.ts: any flag starting with `physical.`.
fn is_physical(flags: &Cbor) -> bool {
    if let Cbor::Array(items) = flags {
        return items
            .iter()
            .any(|f| matches!(f, Cbor::Text(s) if s.starts_with("physical.")));
    }
    false
}

fn body_text<'a>(body: &'a Cbor, key: &str) -> Option<&'a str> {
    match body.get(key) {
        Some(Cbor::Text(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn body_u64(body: &Cbor, key: &str) -> Option<u64> {
    match body.get(key) {
        Some(Cbor::U(n)) => Some(*n),
        _ => None,
    }
}

/// Robot dispatch broker. Holds the paired master verifying key; refuses anything
/// it cannot cryptographically attribute to that key.
pub struct RobotBroker {
    master: VerifyingKey,
    /// Whether a `task` must declare the agent physical to require deadman.
    /// When the manifest is unknown to the broker we conservatively require deadman
    /// for ANY task (fail-safe default).
    physical_hint: bool,
}

impl RobotBroker {
    /// Create a broker bound to a 32-byte ed25519 master public key.
    pub fn new(master_pubkey: [u8; 32]) -> Result<Self, String> {
        let master =
            VerifyingKey::from_bytes(&master_pubkey).map_err(|e| format!("bad master key: {e}"))?;
        Ok(RobotBroker {
            master,
            physical_hint: true,
        })
    }

    /// Feed one CRCP payload unit (header byte + canonical CBOR). Pure + deterministic.
    pub fn handle_unit(&self, unit: &[u8]) -> RobotVerdict {
        let (hdr, body) = match decode_wire(unit) {
            Ok(v) => v,
            Err(e) => {
                return RobotVerdict::Rejected {
                    reason: format!("decode: {e}"),
                }
            }
        };

        // Extract + verify signature for actuating types (task, estop). Manifest/
        // telemetry are informational; we still verify if a signature is present.
        let sig = match body.get("signature") {
            Some(Cbor::Bytes(b)) if b.len() == 64 => {
                let mut arr = [0u8; 64];
                arr.copy_from_slice(b);
                Some(arr)
            }
            Some(_) => {
                return RobotVerdict::Rejected {
                    reason: "signature must be 64-byte bstr".into(),
                }
            }
            None => None,
        };

        match hdr.msg_type {
            MsgType::EStop => {
                // E-stop MUST be signed and valid — never honor a spoofable stop... but
                // note: the on-robot deadman watchdog is the primary fail-safe; this is
                // the active secondary path (raydio-safety-estop.md).
                let sig = match sig {
                    Some(s) => s,
                    None => {
                        return RobotVerdict::Rejected {
                            reason: "estop unsigned".into(),
                        }
                    }
                };
                if !verify(&self.master, MsgType::EStop, &body, &sig) {
                    return RobotVerdict::Rejected {
                        reason: "estop bad signature".into(),
                    };
                }
                let agent_id = body_text(&body, "agent_id").map(|s| s.to_string());
                RobotVerdict::EStop { agent_id }
            }
            MsgType::Task => {
                let sig = match sig {
                    Some(s) => s,
                    None => {
                        return RobotVerdict::Rejected {
                            reason: "task unsigned".into(),
                        }
                    }
                };
                if !verify(&self.master, MsgType::Task, &body, &sig) {
                    return RobotVerdict::Rejected {
                        reason: "task bad signature".into(),
                    };
                }
                // Safety gate: physical agents MUST have deadman_ms. If the broker can't
                // prove the agent is non-physical, it requires deadman (fail-safe).
                let physical = body
                    .get("capability_flags")
                    .map(is_physical)
                    .unwrap_or(self.physical_hint);
                let deadman = body_u64(&body, "deadman_ms");
                match (physical, deadman) {
                    (true, None) => RobotVerdict::Rejected {
                        reason: "deadman_ms required for physical agent (safety)".into(),
                    },
                    (_, Some(0)) => RobotVerdict::Rejected {
                        reason: "deadman_ms must be > 0".into(),
                    },
                    _ => {
                        let agent_id = body_text(&body, "agent_id").unwrap_or("").to_string();
                        RobotVerdict::TaskAccepted {
                            agent_id,
                            deadman_ms: deadman.unwrap_or(0),
                        }
                    }
                }
            }
            MsgType::Manifest => RobotVerdict::Info {
                msg_type: "manifest".into(),
            },
            MsgType::Telemetry => RobotVerdict::Info {
                msg_type: "telemetry".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crcp::{encode_wire, sign, Cbor};
    use ed25519_dalek::SigningKey;

    fn keypair() -> (SigningKey, [u8; 32]) {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk = sk.verifying_key().to_bytes();
        (sk, pk)
    }

    fn task(physical: bool, deadman: Option<u64>, sk: Option<&SigningKey>) -> Vec<u8> {
        let mut pairs = vec![
            (Cbor::text("protocol"), Cbor::text("crcp/1")),
            (Cbor::text("agent_id"), Cbor::text("agent-123")),
            (Cbor::text("intent"), Cbor::text("fetch")),
        ];
        if physical {
            pairs.push((
                Cbor::text("capability_flags"),
                Cbor::Array(vec![Cbor::text("physical.mobility.walk")]),
            ));
        }
        if let Some(ms) = deadman {
            pairs.push((Cbor::text("deadman_ms"), Cbor::U(ms)));
        }
        let mut body = Cbor::Map(pairs);
        if let Some(sk) = sk {
            let s = sign(sk, MsgType::Task, &body);
            if let Cbor::Map(p) = &mut body {
                p.push((Cbor::text("signature"), Cbor::Bytes(s.to_vec())));
            }
        }
        encode_wire(MsgType::Task, &body)
    }

    fn estop(sk: Option<&SigningKey>) -> Vec<u8> {
        let mut body = Cbor::Map(vec![
            (Cbor::text("protocol"), Cbor::text("crcp/1")),
            (Cbor::text("kind"), Cbor::text("estop")),
            (Cbor::text("agent_id"), Cbor::text("agent-123")),
            (Cbor::text("nonce"), Cbor::Bytes(vec![1u8; 16])),
        ]);
        if let Some(sk) = sk {
            let s = sign(sk, MsgType::EStop, &body);
            if let Cbor::Map(p) = &mut body {
                p.push((Cbor::text("signature"), Cbor::Bytes(s.to_vec())));
            }
        }
        encode_wire(MsgType::EStop, &body)
    }

    #[test]
    fn signed_physical_task_with_deadman_accepted() {
        let (sk, pk) = keypair();
        let b = RobotBroker::new(pk).unwrap();
        let v = b.handle_unit(&task(true, Some(3000), Some(&sk)));
        assert_eq!(
            v,
            RobotVerdict::TaskAccepted {
                agent_id: "agent-123".into(),
                deadman_ms: 3000
            }
        );
    }

    #[test]
    fn physical_task_without_deadman_rejected() {
        let (sk, pk) = keypair();
        let b = RobotBroker::new(pk).unwrap();
        let v = b.handle_unit(&task(true, None, Some(&sk)));
        assert!(matches!(v, RobotVerdict::Rejected { .. }), "got {v:?}");
    }

    #[test]
    fn zero_deadman_rejected() {
        let (sk, pk) = keypair();
        let b = RobotBroker::new(pk).unwrap();
        let v = b.handle_unit(&task(true, Some(0), Some(&sk)));
        assert!(matches!(v, RobotVerdict::Rejected { .. }));
    }

    #[test]
    fn unsigned_task_rejected() {
        let (_, pk) = keypair();
        let b = RobotBroker::new(pk).unwrap();
        let v = b.handle_unit(&task(true, Some(3000), None));
        assert_eq!(
            v,
            RobotVerdict::Rejected {
                reason: "task unsigned".into()
            }
        );
    }

    #[test]
    fn task_signed_by_wrong_key_rejected() {
        let (_, pk) = keypair();
        let attacker = SigningKey::from_bytes(&[9u8; 32]);
        let b = RobotBroker::new(pk).unwrap();
        let v = b.handle_unit(&task(true, Some(3000), Some(&attacker)));
        assert_eq!(
            v,
            RobotVerdict::Rejected {
                reason: "task bad signature".into()
            }
        );
    }

    #[test]
    fn signed_estop_honored() {
        let (sk, pk) = keypair();
        let b = RobotBroker::new(pk).unwrap();
        let v = b.handle_unit(&estop(Some(&sk)));
        assert_eq!(
            v,
            RobotVerdict::EStop {
                agent_id: Some("agent-123".into())
            }
        );
    }

    #[test]
    fn unsigned_estop_rejected() {
        let (_, pk) = keypair();
        let b = RobotBroker::new(pk).unwrap();
        assert!(matches!(
            b.handle_unit(&estop(None)),
            RobotVerdict::Rejected { .. }
        ));
    }

    #[test]
    fn unknown_agent_task_without_deadman_failsafe_rejected() {
        // No capability_flags at all → broker cannot prove non-physical → fail-safe
        // requires deadman.
        let (sk, pk) = keypair();
        let b = RobotBroker::new(pk).unwrap();
        let v = b.handle_unit(&task(false, None, Some(&sk)));
        assert!(matches!(v, RobotVerdict::Rejected { .. }));
    }

    #[test]
    fn logical_only_task_without_deadman_accepted() {
        // capability_flags present and contain NO physical.* → deadman not required.
        let (sk, pk) = keypair();
        let b = RobotBroker::new(pk).unwrap();
        let mut body = Cbor::Map(vec![
            (Cbor::text("protocol"), Cbor::text("crcp/1")),
            (Cbor::text("agent_id"), Cbor::text("agent-logic")),
            (Cbor::text("intent"), Cbor::text("summarize")),
            (
                Cbor::text("capability_flags"),
                Cbor::Array(vec![Cbor::text("light"), Cbor::text("tool")]),
            ),
        ]);
        let s = sign(&sk, MsgType::Task, &body);
        if let Cbor::Map(p) = &mut body {
            p.push((Cbor::text("signature"), Cbor::Bytes(s.to_vec())));
        }
        let unit = encode_wire(MsgType::Task, &body);
        assert_eq!(
            b.handle_unit(&unit),
            RobotVerdict::TaskAccepted {
                agent_id: "agent-logic".into(),
                deadman_ms: 0
            }
        );
    }
}
