//! RFC 0001 radio frame: `magic(2) | length(u16 BE) | payload`.
//! Payload = `type(1) | sender(16) | recipient(16) | channel(4) | app_payload`.
//!
//! NOTE: KISS escaping + Reed-Solomon FEC are deferred (RFC 0001 §FEC, TODO);
//! this is the structural framing the daemon and tests build on.

/// CYMRU magic bytes: 'C'=0xC9, 'B'(óg)=0xB0.
pub const MAGIC: [u8; 2] = [0xC9, 0xB0];
/// Broadcast recipient id (all-ones).
pub const BROADCAST: [u8; 16] = [0xFF; 16];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// RFC 0001 Type byte (0x01 data, 0x02 control, 0x03 voice, 0x04 robot-dispatch/CRCP).
    pub type_byte: u8,
    pub sender: [u8; 16],
    pub recipient: [u8; 16],
    pub channel: [u8; 4],
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(
        type_byte: u8,
        sender: [u8; 16],
        recipient: [u8; 16],
        channel: [u8; 4],
        payload: Vec<u8>,
    ) -> Self {
        Frame {
            type_byte,
            sender,
            recipient,
            channel,
            payload,
        }
    }

    pub fn is_broadcast(&self) -> bool {
        self.recipient == BROADCAST
    }

    /// FEC profile for this frame's Type byte (RFC 0001 §FEC):
    /// voice (0x03) gets RS(255,191); everything else RS(255,223).
    pub fn fec_profile(&self) -> crate::fec::FecProfile {
        match self.type_byte {
            0x03 => crate::fec::FecProfile::Voice,
            _ => crate::fec::FecProfile::Data,
        }
    }

    /// Serialize to the on-air byte layout, FEC-protecting the body with the
    /// per-type Reed-Solomon profile. Layout: `magic(2) | rs_len(u16 BE) | RS(body)`
    /// where `body = type | sender | recipient | channel | payload`. The RS stream
    /// length is itself a multiple of 255 (see [`crate::fec`]).
    pub fn encode_fec(&self) -> Vec<u8> {
        let mut body = Vec::with_capacity(1 + 16 + 16 + 4 + self.payload.len());
        body.push(self.type_byte);
        body.extend_from_slice(&self.sender);
        body.extend_from_slice(&self.recipient);
        body.extend_from_slice(&self.channel);
        body.extend_from_slice(&self.payload);

        // FEC can't fail here: body length is far below MAX_PAYLOAD.
        let rs = crate::fec::encode(self.fec_profile(), &body).expect("fec encode");
        let mut out = Vec::with_capacity(4 + rs.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&(rs.len() as u16).to_be_bytes());
        out.extend_from_slice(&rs);
        out
    }

    /// Parse + error-correct an on-air frame produced by [`encode_fec`].
    /// The Type byte determines the RS profile, but it lives *inside* the FEC
    /// stream — so we correct with both profiles and accept the one that yields a
    /// self-consistent Type. (In practice the carrier hints the profile; this keeps
    /// decode self-contained for tests and recovery.)
    pub fn decode_fec(bytes: &[u8]) -> Result<Frame, String> {
        if bytes.len() < 4 {
            return Err("frame too short".into());
        }
        if bytes[0..2] != MAGIC {
            return Err("bad magic".into());
        }
        let rs_len = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
        let rs = bytes.get(4..4 + rs_len).ok_or("truncated fec stream")?;
        // Try Data first (the common case), then Voice.
        let body = crate::fec::decode(crate::fec::FecProfile::Data, rs)
            .or_else(|_| crate::fec::decode(crate::fec::FecProfile::Voice, rs))
            .map_err(|_| "fec uncorrectable".to_string())?;
        if body.len() < 1 + 16 + 16 + 4 {
            return Err("body too short for header".into());
        }
        let type_byte = body[0];
        let mut sender = [0u8; 16];
        sender.copy_from_slice(&body[1..17]);
        let mut recipient = [0u8; 16];
        recipient.copy_from_slice(&body[17..33]);
        let mut channel = [0u8; 4];
        channel.copy_from_slice(&body[33..37]);
        let payload = body[37..].to_vec();
        Ok(Frame {
            type_byte,
            sender,
            recipient,
            channel,
            payload,
        })
    }

    /// Serialize to the on-air byte layout.
    pub fn encode(&self) -> Vec<u8> {
        let mut body = Vec::with_capacity(1 + 16 + 16 + 4 + self.payload.len());
        body.push(self.type_byte);
        body.extend_from_slice(&self.sender);
        body.extend_from_slice(&self.recipient);
        body.extend_from_slice(&self.channel);
        body.extend_from_slice(&self.payload);

        let mut out = Vec::with_capacity(4 + body.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&(body.len() as u16).to_be_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Parse from the on-air byte layout.
    pub fn decode(bytes: &[u8]) -> Result<Frame, String> {
        if bytes.len() < 4 {
            return Err("frame too short".into());
        }
        if bytes[0..2] != MAGIC {
            return Err("bad magic".into());
        }
        let len = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
        let body = bytes.get(4..4 + len).ok_or("truncated body")?;
        if body.len() < 1 + 16 + 16 + 4 {
            return Err("body too short for header".into());
        }
        let type_byte = body[0];
        let mut sender = [0u8; 16];
        sender.copy_from_slice(&body[1..17]);
        let mut recipient = [0u8; 16];
        recipient.copy_from_slice(&body[17..33]);
        let mut channel = [0u8; 4];
        channel.copy_from_slice(&body[33..37]);
        let payload = body[37..].to_vec();
        Ok(Frame {
            type_byte,
            sender,
            recipient,
            channel,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let f = Frame::new(
            crcp::CRCP_PAYLOAD_TYPE,
            [1; 16],
            [2; 16],
            [0, 0, 0, 0],
            vec![9, 8, 7],
        );
        let bytes = f.encode();
        assert_eq!(&bytes[0..2], &MAGIC);
        let g = Frame::decode(&bytes).unwrap();
        assert_eq!(f, g);
        assert_eq!(g.type_byte, 0x04);
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(Frame::decode(&[0x00, 0x00, 0, 0]).is_err());
    }

    #[test]
    fn broadcast_detect() {
        let f = Frame::new(0x04, [1; 16], BROADCAST, [0; 4], vec![]);
        assert!(f.is_broadcast());
    }
}
