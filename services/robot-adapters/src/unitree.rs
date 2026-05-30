//! Unitree adapter — Go2 (12-DOF quadruped) and G1 (humanoid).
//!
//! Real binding: `unitree_sdk2` high-level sport/loco client over **DDS**, reached
//! from the RAYDIO robot-receiver module via Ethernet/UART to the robot's onboard
//! compute. This module only does the vendor-agnostic → Unitree intent mapping and
//! the safety envelope; the DDS call sits behind [`crate::RobotExecutor`].
//!
//! E-stop = **damping mode**: zero velocity then zero-torque/fold so the machine
//! settles safely. This is the legged-robot equivalent of an emergency stop — you
//! do NOT just freeze actuators rigidly, you go compliant.

use crate::{require_i64, AdapterError, VendorAdapter, VendorCommand};
use crcp::Cbor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitreeModel {
    /// Go2 quadruped.
    Go2,
    /// G1 humanoid.
    G1,
}

/// Unitree adapter with a per-model safety envelope.
#[derive(Debug, Clone)]
pub struct UnitreeAdapter {
    pub model: UnitreeModel,
    /// Max body-frame translational speed (m/s) the adapter will pass through.
    pub max_speed_mps: f32,
    /// Max yaw rate (rad/s).
    pub max_yaw_rps: f32,
}

impl UnitreeAdapter {
    /// Sensible default envelope per model.
    pub fn new(model: UnitreeModel) -> Self {
        let (max_speed_mps, max_yaw_rps) = match model {
            UnitreeModel::Go2 => (1.5, 1.5),
            UnitreeModel::G1 => (0.8, 1.0), // humanoid: slower default for stability/safety
        };
        UnitreeAdapter {
            model,
            max_speed_mps,
            max_yaw_rps,
        }
    }
}

fn clamp(v: f32, max: f32) -> f32 {
    v.clamp(-max, max)
}

impl VendorAdapter for UnitreeAdapter {
    fn vendor(&self) -> &'static str {
        "unitree"
    }

    fn translate(&self, intent: &str, params: &Cbor) -> Result<Vec<VendorCommand>, AdapterError> {
        match intent {
            "move" => {
                // Body-frame velocities in mm/s and mrad/s (integer fixed-point).
                let vx = require_i64(params, "vx_mm_s")? as f32 / 1000.0;
                let vy = require_i64(params, "vy_mm_s")? as f32 / 1000.0;
                let yaw = require_i64(params, "yaw_mrad_s")? as f32 / 1000.0;
                Ok(vec![VendorCommand::Move {
                    vx_mps: clamp(vx, self.max_speed_mps),
                    vy_mps: clamp(vy, self.max_speed_mps),
                    yaw_rps: clamp(yaw, self.max_yaw_rps),
                }])
            }
            "stop" => Ok(vec![VendorCommand::Stop]),
            "stand" => Ok(vec![VendorCommand::Stand]),
            "sit" => match self.model {
                UnitreeModel::Go2 => Ok(vec![VendorCommand::Sit]),
                UnitreeModel::G1 => {
                    Err(AdapterError::UnsupportedIntent("sit (G1 humanoid)".into()))
                }
            },
            "pose" => {
                let name = match params.get("name") {
                    Some(Cbor::Text(s)) => s.clone(),
                    _ => return Err(AdapterError::MissingParam("name".into())),
                };
                Ok(vec![VendorCommand::Pose { name }])
            }
            // Aerial intents do not apply to a ground robot.
            "takeoff" | "land" | "hover" | "rth" | "goto" => Err(AdapterError::UnsupportedIntent(
                format!("{intent} (ground robot)"),
            )),
            other => Err(AdapterError::UnsupportedIntent(other.to_string())),
        }
    }

    fn estop(&self) -> Vec<VendorCommand> {
        // Zero velocity, then go compliant (damping). SafeStop = damping for Unitree.
        vec![VendorCommand::Stop, VendorCommand::SafeStop]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crcp::Cbor;

    fn move_params(vx: i64, vy: i64, yaw: i64) -> Cbor {
        Cbor::Map(vec![
            (Cbor::text("vx_mm_s"), Cbor::I(vx)),
            (Cbor::text("vy_mm_s"), Cbor::I(vy)),
            (Cbor::text("yaw_mrad_s"), Cbor::I(yaw)),
        ])
    }

    #[test]
    fn go2_move_translates_and_clamps() {
        let a = UnitreeAdapter::new(UnitreeModel::Go2);
        // 5 m/s requested, clamped to 1.5.
        let cmds = a.translate("move", &move_params(5000, 0, 0)).unwrap();
        assert_eq!(
            cmds,
            vec![VendorCommand::Move {
                vx_mps: 1.5,
                vy_mps: 0.0,
                yaw_rps: 0.0
            }]
        );
    }

    #[test]
    fn g1_slower_envelope() {
        let a = UnitreeAdapter::new(UnitreeModel::G1);
        let cmds = a.translate("move", &move_params(5000, 0, 0)).unwrap();
        assert_eq!(
            cmds,
            vec![VendorCommand::Move {
                vx_mps: 0.8,
                vy_mps: 0.0,
                yaw_rps: 0.0
            }]
        );
    }

    #[test]
    fn go2_sits_g1_does_not() {
        assert_eq!(
            UnitreeAdapter::new(UnitreeModel::Go2).translate("sit", &Cbor::Map(vec![])),
            Ok(vec![VendorCommand::Sit])
        );
        assert!(matches!(
            UnitreeAdapter::new(UnitreeModel::G1).translate("sit", &Cbor::Map(vec![])),
            Err(AdapterError::UnsupportedIntent(_))
        ));
    }

    #[test]
    fn aerial_intent_rejected() {
        let a = UnitreeAdapter::new(UnitreeModel::Go2);
        assert!(matches!(
            a.translate("takeoff", &Cbor::Map(vec![])),
            Err(AdapterError::UnsupportedIntent(_))
        ));
    }

    #[test]
    fn missing_param_rejected() {
        let a = UnitreeAdapter::new(UnitreeModel::Go2);
        assert!(matches!(
            a.translate("move", &Cbor::Map(vec![])),
            Err(AdapterError::MissingParam(_))
        ));
    }

    #[test]
    fn estop_is_stop_then_damping() {
        let a = UnitreeAdapter::new(UnitreeModel::G1);
        assert_eq!(
            a.estop(),
            vec![VendorCommand::Stop, VendorCommand::SafeStop]
        );
    }
}
