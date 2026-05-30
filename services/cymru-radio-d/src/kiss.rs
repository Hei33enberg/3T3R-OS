//! KISS framing — RFC 0001 wraps the on-air packet in KISS (Keep It Simple,
//! Stupid) so a byte-stream transport (UART to the modem, or a noisy SPI FIFO)
//! can find frame boundaries unambiguously.
//!
//! KISS uses one delimiter byte and escapes any collision with it:
//!  - `FEND` (0xC0) marks the start/end of a frame.
//!  - inside the frame, a literal `0xC0` becomes `FESC TFEND` (0xDB 0xDC),
//!    and a literal `0xDB` becomes `FESC TFESC` (0xDB 0xDD).
//!
//! We emit `FEND <escaped bytes> FEND` (leading FEND flushes any line noise the
//! receiver may have buffered — standard KISS practice). The decoder is a small
//! state machine that tolerates leading/trailing noise and surfaces a clean frame.
//!
//! Layering (RFC 0001): KISS is the *outermost* on-air wrapper. Inside it sits the
//! magic+length(+FEC) frame from [`crate::frame`]. KISS here is transport-agnostic
//! bytes-in/bytes-out — it does not know about magic or FEC.

pub const FEND: u8 = 0xC0;
pub const FESC: u8 = 0xDB;
pub const TFEND: u8 = 0xDC;
pub const TFESC: u8 = 0xDD;

/// Wrap a payload in a single KISS frame: `FEND <escaped> FEND`.
pub fn encode(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 2);
    out.push(FEND);
    for &b in payload {
        match b {
            FEND => {
                out.push(FESC);
                out.push(TFEND);
            }
            FESC => {
                out.push(FESC);
                out.push(TFESC);
            }
            other => out.push(other),
        }
    }
    out.push(FEND);
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KissError {
    /// `FESC` was followed by a byte other than `TFEND`/`TFESC`.
    BadEscape,
    /// Stream ended mid-frame (no closing `FEND`) or held no complete frame.
    Incomplete,
}

/// Decode the **first** complete KISS frame found in `stream`, tolerating leading
/// noise and inter-frame `FEND` runs. Returns the unescaped payload.
pub fn decode(stream: &[u8]) -> Result<Vec<u8>, KissError> {
    decode_first(stream).map(|(p, _)| p)
}

/// Like [`decode`], but also returns the index just past the closing `FEND`, so a
/// caller draining a continuous byte stream can resume after a frame.
///
/// Empty frames (a `FEND` immediately followed by `FEND`, i.e. zero payload bytes)
/// are **skipped** — this is standard KISS, and a real on-air frame always carries
/// the magic+length header (≥ 4 bytes), so a zero-length frame is never meaningful.
/// Leading noise before the first `FEND` is also skipped.
pub fn decode_first(stream: &[u8]) -> Result<(Vec<u8>, usize), KissError> {
    let mut i = 0;
    loop {
        // Advance to the next opening FEND, skipping any noise.
        while i < stream.len() && stream[i] != FEND {
            i += 1;
        }
        if i >= stream.len() {
            return Err(KissError::Incomplete);
        }
        i += 1; // consume the opening FEND

        let mut out = Vec::new();
        let mut escaping = false;
        let mut closed = false;
        while i < stream.len() {
            let b = stream[i];
            i += 1;
            if escaping {
                match b {
                    TFEND => out.push(FEND),
                    TFESC => out.push(FESC),
                    _ => return Err(KissError::BadEscape),
                }
                escaping = false;
            } else {
                match b {
                    FESC => escaping = true,
                    FEND => {
                        closed = true;
                        break;
                    }
                    other => out.push(other),
                }
            }
        }
        if !closed {
            return Err(KissError::Incomplete); // ran out before a closing FEND
        }
        if out.is_empty() {
            // Empty frame: rewind so the closing FEND can open the next frame, and
            // keep scanning. (We already advanced past it, so just continue.)
            i -= 1;
            continue;
        }
        return Ok((out, i));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_roundtrip() {
        let p = b"hello radio";
        let enc = encode(p);
        assert_eq!(enc.first(), Some(&FEND));
        assert_eq!(enc.last(), Some(&FEND));
        assert_eq!(decode(&enc).unwrap(), p);
    }

    #[test]
    fn escapes_fend_and_fesc() {
        // Payload contains both special bytes.
        let p = [0x01, FEND, 0x02, FESC, 0x03];
        let enc = encode(&p);
        // No raw FEND in the interior, only the two delimiters.
        assert_eq!(enc.iter().filter(|&&b| b == FEND).count(), 2);
        // FESC TFEND and FESC TFESC sequences present.
        assert!(enc.windows(2).any(|w| w == [FESC, TFEND]));
        assert!(enc.windows(2).any(|w| w == [FESC, TFESC]));
        assert_eq!(decode(&enc).unwrap(), p);
    }

    #[test]
    fn all_special_bytes_roundtrip() {
        let p = [FEND, FESC, TFEND, TFESC, FEND, FESC];
        assert_eq!(decode(&encode(&p)).unwrap(), p);
    }

    #[test]
    fn tolerates_leading_noise_and_fend_runs() {
        let p = b"payload";
        let mut stream = vec![0x55, 0xAA]; // line noise before the frame
        stream.extend_from_slice(&[FEND, FEND, FEND]); // delimiter run
        stream.extend_from_slice(&encode(p)[1..]); // frame without its own leading FEND
        assert_eq!(decode(&stream).unwrap(), p);
    }

    #[test]
    fn empty_frames_are_skipped() {
        // encode("") = [FEND, FEND]; standard KISS treats a zero-length frame as a
        // delimiter run, not a deliverable frame. Real frames carry magic+len (>=4B).
        assert_eq!(encode(b""), vec![FEND, FEND]);
        assert_eq!(decode(&encode(b"")), Err(KissError::Incomplete));
        // An empty frame preceding a real one is skipped; the real one decodes.
        let mut stream = encode(b"");
        stream.extend_from_slice(&encode(b"real"));
        assert_eq!(decode(&stream).unwrap(), b"real");
    }

    #[test]
    fn incomplete_frame_errs() {
        // FEND opens a frame but no closing FEND.
        assert_eq!(decode(&[FEND, 0x01, 0x02]), Err(KissError::Incomplete));
        // No FEND at all.
        assert_eq!(decode(&[0x01, 0x02]), Err(KissError::Incomplete));
    }

    #[test]
    fn bad_escape_errs() {
        // FESC followed by an invalid escapee, inside a frame.
        assert_eq!(decode(&[FEND, FESC, 0x00, FEND]), Err(KissError::BadEscape));
    }

    #[test]
    fn decode_first_reports_resume_index_for_back_to_back_frames() {
        let mut stream = encode(b"one");
        let second = encode(b"two");
        stream.extend_from_slice(&second);
        let (p1, consumed) = decode_first(&stream).unwrap();
        assert_eq!(p1, b"one");
        let (p2, _) = decode_first(&stream[consumed..]).unwrap();
        assert_eq!(p2, b"two");
    }
}
