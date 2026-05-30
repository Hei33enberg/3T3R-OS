//! robot-adapters — translate vendor-agnostic CRCP intents into concrete vendor
//! commands for **Unitree** (Go2 quadruped / G1 humanoid) and **DJI** drones.
//!
//! Where this sits: the RAYDIO robot-receiver module (LINEAR-2538) receives a
//! CRCP `TaskEnvelope` / `EStopFrame` over radio, the `cymru-bridge-d` RobotBroker
//! verifies the ed25519 signature + safety gate (deadman), and *then* an adapter
//! here turns the validated, vendor-agnostic intent into the robot's native API.
//!
//! Design rules:
//! - This crate depends only on `crcp` (the schema source of truth). It never
//!   re-defines CRCP types.
//! - **Params are integer fixed-point** (mm/s, mrad/s, micro-degrees, cm). Floats
//!   are avoided on the wire because deterministic CBOR (RFC 8949 §4.2, used for
//!   the signing pre-image) has no canonical float form. Adapters convert to SI.
//! - The real SDK binding (Unitree DDS `unitree_sdk2`, DJI Payload SDK) lives
//!   behind [`RobotExecutor`]; until hardware, use [`RecordingExecutor`].
//! - **Safety is vendor-specific and non-negotiable.** [`VendorAdapter::estop`]
//!   returns the *correct safe state for that machine*: legged → damping; aerial →
//!   hover (NEVER motor-kill in air). See [`unitree`] / [`dji`].

use crcp::Cbor;

pub mod dji;
pub mod unitree;

pub use dji::DjiAdapter;
pub use unitree::{UnitreeAdapter, UnitreeModel};

/// A normalized, vendor-neutral robot command. Adapters map CRCP intents to a
/// sequence of these; an executor turns them into native SDK calls.
#[derive(Debug, Clone, PartialEq)]
pub enum VendorCommand {
    /// Ground locomotion: body-frame velocities (SI).
    Move {
        vx_mps: f32,
        vy_mps: f32,
        yaw_rps: f32,
    },
    /// Controlled stop in place (zero velocity, hold).
    Stop,
    /// Legged: stand up / balance.
    Stand,
    /// Legged: sit / fold down.
    Sit,
    /// Named pose / gesture (vendor-specific name passed through).
    Pose { name: String },
    /// Aerial: take off to hover.
    Takeoff,
    /// Aerial: land in place.
    Land,
    /// Aerial: hold position (the safe immediate stop for a flying machine).
    Hover,
    /// Aerial: return to launch / home.
    ReturnToHome,
    /// Aerial: fly to a GPS waypoint.
    GotoGps {
        lat_deg: f64,
        lon_deg: f64,
        alt_m: f32,
    },
    /// Vendor-specific *safe state*: Unitree → damping (zero-torque/lie); DJI →
    /// hover (and let the flight-controller failsafe RTH on persistent link loss).
    SafeStop,
}

/// Why an intent could not be translated. NEVER actuate on an error.
#[derive(Debug, Clone, PartialEq)]
pub enum AdapterError {
    /// The vendor/model does not support this intent (e.g. `takeoff` to a quadruped).
    UnsupportedIntent(String),
    /// A required parameter was missing or the wrong CBOR type.
    MissingParam(String),
    /// A parameter was present but outside the safe envelope (e.g. altitude > ceiling).
    OutOfRange(String),
}

/// A vendor adapter: turns a vendor-agnostic CRCP intent into native commands and
/// owns the vendor-correct emergency-stop behavior.
pub trait VendorAdapter {
    /// Stable vendor identifier (e.g. `"unitree"`, `"dji"`).
    fn vendor(&self) -> &'static str;

    /// Translate one CRCP intent + params map into a command sequence.
    fn translate(&self, intent: &str, params: &Cbor) -> Result<Vec<VendorCommand>, AdapterError>;

    /// The emergency-stop sequence that is *safe for this machine right now*.
    /// Must never produce a command that endangers the platform (e.g. cutting
    /// motors on an airborne drone).
    fn estop(&self) -> Vec<VendorCommand>;
}

/// Executes normalized commands on a real (or mock) robot. The real Unitree DDS /
/// DJI PSDK binding implements this; tests use [`RecordingExecutor`].
pub trait RobotExecutor {
    fn execute(&mut self, cmd: &VendorCommand) -> Result<(), String>;
}

/// In-memory executor that records what it was asked to do (tests / dry-run).
#[derive(Debug, Default)]
pub struct RecordingExecutor {
    pub log: Vec<VendorCommand>,
}

impl RobotExecutor for RecordingExecutor {
    fn execute(&mut self, cmd: &VendorCommand) -> Result<(), String> {
        self.log.push(cmd.clone());
        Ok(())
    }
}

/// Execute a command sequence in order, stopping at the first failure.
pub fn run<E: RobotExecutor>(exec: &mut E, cmds: &[VendorCommand]) -> Result<(), String> {
    for c in cmds {
        exec.execute(c)?;
    }
    Ok(())
}

// ── shared param helpers (integer fixed-point on the wire → SI) ──────────────

/// Read an integer param (accepts CBOR unsigned or negative).
pub(crate) fn param_i64(params: &Cbor, key: &str) -> Option<i64> {
    match params.get(key) {
        Some(Cbor::I(n)) => Some(*n),
        Some(Cbor::U(n)) => i64::try_from(*n).ok(),
        _ => None,
    }
}

/// Read a required integer param or fail with [`AdapterError::MissingParam`].
pub(crate) fn require_i64(params: &Cbor, key: &str) -> Result<i64, AdapterError> {
    param_i64(params, key).ok_or_else(|| AdapterError::MissingParam(key.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_executor_runs_sequence() {
        let mut e = RecordingExecutor::default();
        run(&mut e, &[VendorCommand::Stand, VendorCommand::Stop]).unwrap();
        assert_eq!(e.log, vec![VendorCommand::Stand, VendorCommand::Stop]);
    }

    #[test]
    fn param_helpers_accept_signed_and_unsigned() {
        let p = Cbor::Map(vec![
            (Cbor::text("a"), Cbor::U(5)),
            (Cbor::text("b"), Cbor::I(-7)),
        ]);
        assert_eq!(param_i64(&p, "a"), Some(5));
        assert_eq!(param_i64(&p, "b"), Some(-7));
        assert_eq!(param_i64(&p, "missing"), None);
        assert!(require_i64(&p, "missing").is_err());
    }
}
