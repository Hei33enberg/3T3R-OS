//! End-to-end: a CRCP `0x04` payload unit (exactly what arrives over radio inside
//! an RFC 0001 frame) → decode → pick the vendor adapter → translate intent →
//! execute on a recording executor. Proves the radio→robot control path with no
//! hardware. The ed25519 auth + deadman safety gate live in `cymru-bridge-d`
//! (RobotBroker) upstream of this; here we prove the translation/dispatch half.

use crcp::{decode_wire, encode_wire, Cbor, MsgType};
use robot_adapters::{
    run, DjiAdapter, RecordingExecutor, UnitreeAdapter, UnitreeModel, VendorAdapter, VendorCommand,
};

/// Build a CRCP Task wire unit carrying an intent + params (as the radio would).
fn task_unit(intent: &str, params: Vec<(Cbor, Cbor)>) -> Vec<u8> {
    let body = Cbor::Map(vec![
        (Cbor::text("protocol"), Cbor::text("crcp/1")),
        (Cbor::text("agent_id"), Cbor::text("agent-xyz")),
        (Cbor::text("intent"), Cbor::text(intent)),
        (Cbor::text("params"), Cbor::Map(params)),
    ]);
    encode_wire(MsgType::Task, &body)
}

/// Extract (intent, params) from a decoded CRCP task body.
fn intent_and_params(body: &Cbor) -> (String, Cbor) {
    let intent = match body.get("intent") {
        Some(Cbor::Text(s)) => s.clone(),
        _ => String::new(),
    };
    let params = body.get("params").cloned().unwrap_or(Cbor::Map(vec![]));
    (intent, params)
}

#[test]
fn unitree_go2_walk_forward_over_the_wire() {
    // "walk forward 0.5 m/s" arrives as a CRCP 0x04 unit.
    let unit = task_unit(
        "move",
        vec![
            (Cbor::text("vx_mm_s"), Cbor::I(500)),
            (Cbor::text("vy_mm_s"), Cbor::I(0)),
            (Cbor::text("yaw_mrad_s"), Cbor::I(0)),
        ],
    );
    let (hdr, body) = decode_wire(&unit).unwrap();
    assert_eq!(hdr.msg_type, MsgType::Task);

    let (intent, params) = intent_and_params(&body);
    let adapter = UnitreeAdapter::new(UnitreeModel::Go2);
    let cmds = adapter.translate(&intent, &params).unwrap();

    let mut exec = RecordingExecutor::default();
    run(&mut exec, &cmds).unwrap();
    assert_eq!(
        exec.log,
        vec![VendorCommand::Move {
            vx_mps: 0.5,
            vy_mps: 0.0,
            yaw_rps: 0.0
        }]
    );
}

#[test]
fn dji_takeoff_then_goto_over_the_wire() {
    let adapter = DjiAdapter::new();
    let mut exec = RecordingExecutor::default();

    // takeoff
    let (_, body) = decode_wire(&task_unit("takeoff", vec![])).unwrap();
    let (intent, params) = intent_and_params(&body);
    run(&mut exec, &adapter.translate(&intent, &params).unwrap()).unwrap();

    // goto a waypoint at 50 m
    let unit = task_unit(
        "goto",
        vec![
            (Cbor::text("lat_udeg"), Cbor::I(52_229_676)),
            (Cbor::text("lon_udeg"), Cbor::I(21_012_229)),
            (Cbor::text("alt_cm"), Cbor::I(5000)),
        ],
    );
    let (_, body) = decode_wire(&unit).unwrap();
    let (intent, params) = intent_and_params(&body);
    run(&mut exec, &adapter.translate(&intent, &params).unwrap()).unwrap();

    assert_eq!(exec.log.len(), 2);
    assert_eq!(exec.log[0], VendorCommand::Takeoff);
    assert!(matches!(exec.log[1], VendorCommand::GotoGps { .. }));
}

#[test]
fn dji_estop_is_hover_not_kill_end_to_end() {
    // Whatever the intent was, an e-stop must yield Hover for a drone.
    let adapter = DjiAdapter::new();
    let mut exec = RecordingExecutor::default();
    run(&mut exec, &adapter.estop()).unwrap();
    assert_eq!(exec.log, vec![VendorCommand::Hover]);
}

#[test]
fn unitree_estop_is_stop_then_damping_end_to_end() {
    let adapter = UnitreeAdapter::new(UnitreeModel::G1);
    let mut exec = RecordingExecutor::default();
    run(&mut exec, &adapter.estop()).unwrap();
    assert_eq!(exec.log, vec![VendorCommand::Stop, VendorCommand::SafeStop]);
}

#[test]
fn wrong_vendor_for_intent_is_rejected_end_to_end() {
    // Sending "takeoff" to a Unitree quadruped must be refused, not mis-executed.
    let (_, body) = decode_wire(&task_unit("takeoff", vec![])).unwrap();
    let (intent, params) = intent_and_params(&body);
    let adapter = UnitreeAdapter::new(UnitreeModel::Go2);
    assert!(adapter.translate(&intent, &params).is_err());
}
