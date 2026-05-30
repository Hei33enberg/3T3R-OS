//! Radio backend trait + a cross-platform mock (loopback) for tests/dev.
//! The real SX1262 SPI backend (Linux-only) implements the same [`Radio`] trait.

use crate::frame::Frame;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// Abstract radio transport: send/receive raw on-air bytes.
pub trait Radio {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn recv(&mut self) -> Option<Vec<u8>>;
}

/// In-memory loopback "ether" shared by mock radios (dev + tests, no hardware).
#[derive(Clone, Default)]
pub struct MockEther {
    buf: Rc<RefCell<VecDeque<Vec<u8>>>>,
}

impl MockEther {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A mock radio endpoint over a shared [`MockEther`].
pub struct MockRadio {
    ether: MockEther,
}

impl MockRadio {
    pub fn new(ether: MockEther) -> Self {
        MockRadio { ether }
    }
}

impl Radio for MockRadio {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.ether.buf.borrow_mut().push_back(bytes.to_vec());
        Ok(())
    }
    fn recv(&mut self) -> Option<Vec<u8>> {
        self.ether.buf.borrow_mut().pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Frame, BROADCAST};
    use crcp::{decode_wire, encode_wire, Cbor, MsgType, CRCP_PAYLOAD_TYPE};

    #[test]
    fn crcp_unit_over_frame_over_mock_radio() {
        // Build a CRCP telemetry body, wrap as 0x04 payload, frame it, send over mock ether.
        let body = Cbor::Map(vec![
            (Cbor::text("protocol"), Cbor::text("crcp/1")),
            (Cbor::text("status"), Cbor::text("completed")),
        ]);
        let unit = encode_wire(MsgType::Telemetry, &body);
        let frame = Frame::new(CRCP_PAYLOAD_TYPE, [1; 16], BROADCAST, [0; 4], unit);
        let on_air = frame.encode();

        let ether = MockEther::new();
        let mut tx = MockRadio::new(ether.clone());
        let mut rx = MockRadio::new(ether);

        tx.send(&on_air).unwrap();
        let got = rx.recv().expect("delivered");
        let decoded_frame = Frame::decode(&got).unwrap();
        assert_eq!(decoded_frame.type_byte, CRCP_PAYLOAD_TYPE);
        let (hdr, decoded_body) = decode_wire(&decoded_frame.payload).unwrap();
        assert_eq!(hdr.msg_type, MsgType::Telemetry);
        assert_eq!(decoded_body.get("status").unwrap(), &Cbor::text("completed"));
    }
}
