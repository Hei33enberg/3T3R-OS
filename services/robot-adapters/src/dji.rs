//! DJI adapter — drones (e.g. Matrice / Mavic Enterprise with onboard SDK).
//!
//! Integration reality (be honest, like the Reticulum call): you do NOT impersonate
//! DJI's OcuSync RF link. Control happens through the **DJI Payload SDK (PSDK)** /
//! Onboard SDK running on a companion computer wired to the aircraft (E-Port/UART),
//! or MSDK on a connected device. The RAYDIO robot-receiver module talks to that
//! PSDK companion; this module only maps vendor-agnostic intents to flight commands
//! behind [`crate::RobotExecutor`]. (Some markets/firmwares restrict autonomous
//! control — that's a per-deployment compliance check, not a code concern here.)
//!
//! ⚠️ SAFETY — the load-bearing difference vs a ground robot: an emergency stop on
//! a flying machine MUST NOT cut motors (that is a crash). [`DjiAdapter::estop`]
//! commands **Hover** (immediate brake-in-place); persistent link loss is then
//! handled by the flight controller's own failsafe RTH. Motor-kill is refused.

use crate::{param_i64, require_i64, AdapterError, VendorAdapter, VendorCommand};
use crcp::Cbor;

/// DJI drone adapter with a flight safety envelope.
#[derive(Debug, Clone)]
pub struct DjiAdapter {
    /// Max horizontal speed (m/s) passed through for velocity moves.
    pub max_speed_mps: f32,
    /// Altitude ceiling (m AGL). `goto`/`takeoff` above this is rejected.
    pub max_alt_m: f32,
}

impl DjiAdapter {
    pub fn new() -> Self {
        // Conservative defaults; tune per airframe + local regulation.
        DjiAdapter {
            max_speed_mps: 10.0,
            max_alt_m: 120.0,
        }
    }
}

impl Default for DjiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

fn clamp(v: f32, max: f32) -> f32 {
    v.clamp(-max, max)
}

impl VendorAdapter for DjiAdapter {
    fn vendor(&self) -> &'static str {
        "dji"
    }

    fn translate(&self, intent: &str, params: &Cbor) -> Result<Vec<VendorCommand>, AdapterError> {
        match intent {
            "takeoff" => Ok(vec![VendorCommand::Takeoff]),
            "land" => Ok(vec![VendorCommand::Land]),
            "hover" => Ok(vec![VendorCommand::Hover]),
            "rth" => Ok(vec![VendorCommand::ReturnToHome]),
            "goto" => {
                // GPS in micro-degrees, altitude in cm (integer fixed-point).
                let lat = require_i64(params, "lat_udeg")? as f64 / 1_000_000.0;
                let lon = require_i64(params, "lon_udeg")? as f64 / 1_000_000.0;
                let alt_m = require_i64(params, "alt_cm")? as f32 / 100.0;
                if !(-90.0..=90.0).contains(&lat) {
                    return Err(AdapterError::OutOfRange(format!("lat {lat}")));
                }
                if !(-180.0..=180.0).contains(&lon) {
                    return Err(AdapterError::OutOfRange(format!("lon {lon}")));
                }
                if alt_m <= 0.0 || alt_m > self.max_alt_m {
                    return Err(AdapterError::OutOfRange(format!(
                        "alt {alt_m} m (ceiling {})",
                        self.max_alt_m
                    )));
                }
                Ok(vec![VendorCommand::GotoGps {
                    lat_deg: lat,
                    lon_deg: lon,
                    alt_m,
                }])
            }
            "move" => {
                // Horizontal body-frame velocity in mm/s; yaw in mrad/s. vz omitted
                // here (climb handled via goto/takeoff) to keep the envelope simple.
                let vx = require_i64(params, "vx_mm_s")? as f32 / 1000.0;
                let vy = require_i64(params, "vy_mm_s")? as f32 / 1000.0;
                let yaw = param_i64(params, "yaw_mrad_s").unwrap_or(0) as f32 / 1000.0;
                Ok(vec![VendorCommand::Move {
                    vx_mps: clamp(vx, self.max_speed_mps),
                    vy_mps: clamp(vy, self.max_speed_mps),
                    yaw_rps: yaw,
                }])
            }
            // Ground/legged intents do not apply to an aircraft.
            "stand" | "sit" | "pose" => Err(AdapterError::UnsupportedIntent(format!(
                "{intent} (aircraft)"
            ))),
            // Explicitly refuse motor-kill: catastrophic in flight.
            "motor_kill" | "kill" => Err(AdapterError::OutOfRange(
                "motor-kill refused: would crash an airborne aircraft; use estop (hover) / rth"
                    .into(),
            )),
            other => Err(AdapterError::UnsupportedIntent(other.to_string())),
        }
    }

    fn estop(&self) -> Vec<VendorCommand> {
        // Immediate brake-in-place. NEVER motor-kill in air. The flight controller's
        // own failsafe escalates to RTH on persistent link loss (independent path,
        // analogous to the on-robot deadman watchdog).
        vec![VendorCommand::Hover]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crcp::Cbor;

    #[test]
    fn flight_primitives() {
        let a = DjiAdapter::new();
        assert_eq!(
            a.translate("takeoff", &Cbor::Map(vec![])),
            Ok(vec![VendorCommand::Takeoff])
        );
        assert_eq!(
            a.translate("land", &Cbor::Map(vec![])),
            Ok(vec![VendorCommand::Land])
        );
        assert_eq!(
            a.translate("rth", &Cbor::Map(vec![])),
            Ok(vec![VendorCommand::ReturnToHome])
        );
    }

    #[test]
    fn goto_parses_fixed_point_gps() {
        let a = DjiAdapter::new();
        let p = Cbor::Map(vec![
            (Cbor::text("lat_udeg"), Cbor::I(52_229_676)), // 52.229676
            (Cbor::text("lon_udeg"), Cbor::I(21_012_229)), // 21.012229
            (Cbor::text("alt_cm"), Cbor::I(5000)),         // 50.0 m
        ]);
        let cmds = a.translate("goto", &p).unwrap();
        match &cmds[0] {
            VendorCommand::GotoGps {
                lat_deg,
                lon_deg,
                alt_m,
            } => {
                assert!((lat_deg - 52.229676).abs() < 1e-6);
                assert!((lon_deg - 21.012229).abs() < 1e-6);
                assert_eq!(*alt_m, 50.0);
            }
            other => panic!("expected GotoGps, got {other:?}"),
        }
    }

    #[test]
    fn altitude_ceiling_enforced() {
        let a = DjiAdapter::new();
        let p = Cbor::Map(vec![
            (Cbor::text("lat_udeg"), Cbor::I(0)),
            (Cbor::text("lon_udeg"), Cbor::I(0)),
            (Cbor::text("alt_cm"), Cbor::I(50_000)), // 500 m > 120 m ceiling
        ]);
        assert!(matches!(
            a.translate("goto", &p),
            Err(AdapterError::OutOfRange(_))
        ));
    }

    #[test]
    fn estop_is_hover_never_kill() {
        let a = DjiAdapter::new();
        assert_eq!(a.estop(), vec![VendorCommand::Hover]);
    }

    #[test]
    fn motor_kill_refused() {
        let a = DjiAdapter::new();
        assert!(matches!(
            a.translate("motor_kill", &Cbor::Map(vec![])),
            Err(AdapterError::OutOfRange(_))
        ));
    }

    #[test]
    fn ground_intent_rejected() {
        let a = DjiAdapter::new();
        assert!(matches!(
            a.translate("sit", &Cbor::Map(vec![])),
            Err(AdapterError::UnsupportedIntent(_))
        ));
    }
}
