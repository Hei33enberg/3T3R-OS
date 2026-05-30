//! Forward error correction — Reed-Solomon per RFC 0001 §FEC.
//!
//! LoRa links are lossy and HF is lossier; RFC 0001 specifies byte-oriented
//! Reed-Solomon over GF(2^8), block length 255:
//!  - **RS(255,223)** for data/control — 32 ecc bytes, corrects up to 16 byte
//!    errors per block (~12% overhead).
//!  - **RS(255,191)** for voice — 64 ecc bytes, corrects up to 32 byte errors per
//!    block (~25% overhead), for HF Codec2 where fading is worse.
//!
//! We do NOT hand-roll the GF math — that is exactly where a "half-correct" codec
//! bites. We use the `reed-solomon` crate (Berlekamp-Massey decoder) and own only
//! the blocking/length-framing around it.
//!
//! Framing: the logical payload is prefixed with a 2-byte big-endian length, then
//! split into `k`-byte data chunks (last zero-padded). Each chunk is RS-encoded to
//! a 255-byte block; blocks are concatenated. The length prefix lives inside block
//! 0's data region, so it is itself FEC-protected. On decode every block is
//! corrected, data regions are concatenated, and the first 2 bytes give the true
//! payload length (padding trimmed). Encoded stream length is always a multiple of
//! 255 — a non-multiple is a hard framing error.

use reed_solomon::{Decoder, Encoder};

/// FEC strength selector (maps to RFC 0001 payload Type / safety_class).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FecProfile {
    /// RS(255,223): data/control. 32 ecc, corrects ≤16 byte errors/block.
    Data,
    /// RS(255,191): voice / critical. 64 ecc, corrects ≤32 byte errors/block.
    Voice,
}

impl FecProfile {
    /// Number of ECC (parity) bytes per 255-byte block.
    pub const fn ecc_len(self) -> usize {
        match self {
            FecProfile::Data => 32,  // 255 - 223
            FecProfile::Voice => 64, // 255 - 191
        }
    }
    /// Data bytes per block (`k` = 255 - ecc).
    pub const fn block_data_len(self) -> usize {
        255 - self.ecc_len()
    }
    /// Max correctable byte errors per block (`ecc / 2`).
    pub const fn correctable(self) -> usize {
        self.ecc_len() / 2
    }
}

/// Maximum logical payload this codec frames (u16 length prefix).
pub const MAX_PAYLOAD: usize = u16::MAX as usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FecError {
    /// Payload exceeds the u16 length prefix capacity.
    TooLarge,
    /// Encoded stream is not a whole number of 255-byte blocks.
    BadBlockAlignment,
    /// A block had more errors than the profile can correct.
    Uncorrectable,
    /// Decoded length prefix is inconsistent with the recovered data.
    BadLength,
}

/// Encode `payload` with the given FEC profile. Output length is a multiple of 255.
pub fn encode(profile: FecProfile, payload: &[u8]) -> Result<Vec<u8>, FecError> {
    if payload.len() > MAX_PAYLOAD {
        return Err(FecError::TooLarge);
    }
    let k = profile.block_data_len();
    let enc = Encoder::new(profile.ecc_len());

    // length prefix (u16 BE) + payload, then pad to a whole number of k-chunks.
    let mut logical = Vec::with_capacity(2 + payload.len());
    logical.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    logical.extend_from_slice(payload);
    while logical.len() % k != 0 {
        logical.push(0);
    }

    let mut out = Vec::with_capacity(logical.len() / k * 255);
    for chunk in logical.chunks(k) {
        // `encode` produces data+ecc; chunk is exactly k bytes so block == 255.
        let block = enc.encode(chunk);
        out.extend_from_slice(&block[..]);
    }
    Ok(out)
}

/// Decode + error-correct an FEC stream produced by [`encode`].
pub fn decode(profile: FecProfile, encoded: &[u8]) -> Result<Vec<u8>, FecError> {
    if encoded.is_empty() || encoded.len() % 255 != 0 {
        return Err(FecError::BadBlockAlignment);
    }
    let k = profile.block_data_len();
    let dec = Decoder::new(profile.ecc_len());

    let mut logical = Vec::with_capacity(encoded.len() / 255 * k);
    for block in encoded.chunks(255) {
        let mut buf = block.to_vec();
        let recovered = dec
            .correct(&mut buf, None)
            .map_err(|_| FecError::Uncorrectable)?;
        logical.extend_from_slice(recovered.data());
    }

    if logical.len() < 2 {
        return Err(FecError::BadLength);
    }
    let len = u16::from_be_bytes([logical[0], logical[1]]) as usize;
    if 2 + len > logical.len() {
        return Err(FecError::BadLength);
    }
    Ok(logical[2..2 + len].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_params_match_rfc0001() {
        assert_eq!(FecProfile::Data.ecc_len(), 32);
        assert_eq!(FecProfile::Data.block_data_len(), 223);
        assert_eq!(FecProfile::Data.correctable(), 16);
        assert_eq!(FecProfile::Voice.ecc_len(), 64);
        assert_eq!(FecProfile::Voice.block_data_len(), 191);
        assert_eq!(FecProfile::Voice.correctable(), 32);
    }

    #[test]
    fn clean_roundtrip_data() {
        let payload = b"hello mesh, this is a CRCP-ish payload over a lossy LoRa link";
        let enc = encode(FecProfile::Data, payload).unwrap();
        assert_eq!(enc.len() % 255, 0);
        assert_eq!(decode(FecProfile::Data, &enc).unwrap(), payload);
    }

    #[test]
    fn corrects_up_to_16_errors_per_block_data() {
        let payload = vec![0x5Au8; 100]; // single block
        let mut enc = encode(FecProfile::Data, &payload).unwrap();
        assert_eq!(enc.len(), 255);
        // Flip exactly 16 bytes in the block — RS(255,223) must recover.
        for i in 0..16 {
            enc[i * 15] ^= 0xFF;
        }
        assert_eq!(decode(FecProfile::Data, &enc).unwrap(), payload);
    }

    #[test]
    fn fails_beyond_correction_capacity_data() {
        let payload = vec![0x5Au8; 100];
        let mut enc = encode(FecProfile::Data, &payload).unwrap();
        // 17 byte errors > 16 capacity → must fail (not silently mis-decode).
        for i in 0..17 {
            enc[i * 14] ^= 0xFF;
        }
        assert_eq!(decode(FecProfile::Data, &enc), Err(FecError::Uncorrectable));
    }

    #[test]
    fn voice_corrects_up_to_32_errors_per_block() {
        let payload = vec![0xA5u8; 120];
        let mut enc = encode(FecProfile::Voice, &payload).unwrap();
        assert_eq!(enc.len(), 255);
        for i in 0..32 {
            enc[i * 7] ^= 0xFF;
        }
        assert_eq!(decode(FecProfile::Voice, &enc).unwrap(), payload);
    }

    #[test]
    fn multi_block_payload_with_errors_in_each_block() {
        // 500 bytes → 3 data blocks (223*2 = 446 < 502 ≤ 669).
        let payload: Vec<u8> = (0..500).map(|i| (i % 256) as u8).collect();
        let mut enc = encode(FecProfile::Data, &payload).unwrap();
        assert_eq!(enc.len(), 3 * 255);
        // Corrupt 10 bytes in each of the 3 blocks.
        for blk in 0..3 {
            for i in 0..10 {
                enc[blk * 255 + i * 20] ^= 0xFF;
            }
        }
        assert_eq!(decode(FecProfile::Data, &enc).unwrap(), payload);
    }

    #[test]
    fn empty_payload_roundtrips() {
        let enc = encode(FecProfile::Data, b"").unwrap();
        assert_eq!(decode(FecProfile::Data, &enc).unwrap(), b"");
    }

    #[test]
    fn misaligned_stream_rejected() {
        assert_eq!(
            decode(FecProfile::Data, &[0u8; 100]),
            Err(FecError::BadBlockAlignment)
        );
        assert_eq!(
            decode(FecProfile::Data, &[]),
            Err(FecError::BadBlockAlignment)
        );
    }
}
