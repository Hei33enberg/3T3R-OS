//! cymru-radio-d entrypoint (C3). Demonstrates the framing + queue + mock radio
//! pipeline end-to-end without hardware. Real SX1262 SPI loop lands behind the
//! `Radio` trait (Linux-only) in a follow-up.

use cymru_radio_d::{Frame, MockEther, MockRadio, Priority, Radio, TxQueue, BROADCAST};

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(
        "cymru-radio-d {} starting (mock backend; SX1262 TODO)",
        env!("CARGO_PKG_VERSION")
    );

    let ether = MockEther::new();
    let mut tx = MockRadio::new(ether.clone());
    let mut rx = MockRadio::new(ether);

    let mut q = TxQueue::new();
    q.push(
        Priority::Data,
        Frame::new(0x01, [1; 16], BROADCAST, [0; 4], b"hello mesh".to_vec()),
    );
    q.push(
        Priority::EStop,
        Frame::new(0x04, [9; 16], BROADCAST, [0xF0, 0, 0, 0], b"ESTOP".to_vec()),
    );

    while let Some(frame) = q.pop() {
        tx.send(&frame.encode()).unwrap();
    }
    while let Some(bytes) = rx.recv() {
        match Frame::decode(&bytes) {
            Ok(f) => tracing::info!(
                "rx type=0x{:02x} {} bytes payload",
                f.type_byte,
                f.payload.len()
            ),
            Err(e) => tracing::warn!("drop: {e}"),
        }
    }
    tracing::info!("cymru-radio-d demo complete");
}
