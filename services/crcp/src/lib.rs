//! CRCP wire-format codec — RFC 0003 (cymru-os/docs/rfcs/0003-crcp.md).
//!
//! Implements ONLY the wire-format + signing pre-image. The *schema* (the four
//! CRCP contracts) is the source of truth in `cymru-main/src/lib/agents/crcp.ts`
//! and its reference codec `crcp-wire.ts`; this crate is the radio-side mirror.
//!
//! Pieces:
//! - deterministic CBOR (RFC 8949 §4.2): [`Cbor`], [`encode_canonical`], [`decode`]
//! - signing pre-image: `"CRCP1" || selector || detCBOR(body \ signature)` ([`preimage`])
//! - ed25519 sign/verify over that pre-image ([`sign`], [`verify`])
//! - 0x04 payload unit header (ver/frag/2-bit selector) ([`encode_wire`], [`decode_wire`])
//! - fragmentation (>`MAX_CRCP_FRAGMENT`) + reassembly ([`fragment_unit`], [`Reassembler`])
//! - e-stop never fragments ([`estop_fits_single_packet`])

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// RFC 0001 KISS Type byte for robot-dispatch (CRCP).
pub const CRCP_PAYLOAD_TYPE: u8 = 0x04;
/// CRCP major version (high nibble of the header byte).
pub const CRCP_VERSION: u8 = 0x1;
/// Domain-separation tag prepended to every signing pre-image.
pub const DOMAIN_TAG: &[u8] = b"CRCP1";
/// Max CBOR body bytes before fragmentation (RFC 0003 §4).
pub const MAX_CRCP_FRAGMENT: usize = 200;

/// CRCP message kind ↔ 2-bit selector in the header byte / `CrcpMessage` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Manifest = 0,
    Task = 1,
    Telemetry = 2,
    EStop = 3,
}
impl MsgType {
    pub fn from_selector(s: u8) -> Option<MsgType> {
        match s & 0x3 {
            0 => Some(MsgType::Manifest),
            1 => Some(MsgType::Task),
            2 => Some(MsgType::Telemetry),
            3 => Some(MsgType::EStop),
            _ => None,
        }
    }
    pub fn selector(self) -> u8 {
        self as u8
    }
}

// ─────────────────────────── Deterministic CBOR ───────────────────────────

/// Minimal CBOR value supporting exactly the subset CRCP needs.
#[derive(Debug, Clone, PartialEq)]
pub enum Cbor {
    U(u64),
    I(i64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Cbor>),
    /// Map as key/value pairs; canonical ordering is applied at encode time.
    Map(Vec<(Cbor, Cbor)>),
    Bool(bool),
    Null,
    Tag(u64, Box<Cbor>),
}

impl Cbor {
    pub fn text(s: &str) -> Cbor {
        Cbor::Text(s.to_string())
    }
    /// Look up a string key in a map value.
    pub fn get(&self, key: &str) -> Option<&Cbor> {
        if let Cbor::Map(pairs) = self {
            for (k, v) in pairs {
                if let Cbor::Text(s) = k {
                    if s == key {
                        return Some(v);
                    }
                }
            }
        }
        None
    }
    /// Return a copy of this map with the given string key removed (used to strip `signature`).
    pub fn without_key(&self, key: &str) -> Cbor {
        if let Cbor::Map(pairs) = self {
            let kept = pairs
                .iter()
                .filter(|(k, _)| !matches!(k, Cbor::Text(s) if s == key))
                .cloned()
                .collect();
            Cbor::Map(kept)
        } else {
            self.clone()
        }
    }
}

fn write_head(major: u8, n: u64, out: &mut Vec<u8>) {
    let m = major << 5;
    if n < 24 {
        out.push(m | (n as u8));
    } else if n < 0x100 {
        out.push(m | 24);
        out.push(n as u8);
    } else if n < 0x1_0000 {
        out.push(m | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n < 0x1_0000_0000 {
        out.push(m | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

fn enc(c: &Cbor, out: &mut Vec<u8>) {
    match c {
        Cbor::U(n) => write_head(0, *n, out),
        Cbor::I(n) => {
            if *n < 0 {
                write_head(1, (-1 - *n) as u64, out);
            } else {
                write_head(0, *n as u64, out);
            }
        }
        Cbor::Bytes(b) => {
            write_head(2, b.len() as u64, out);
            out.extend_from_slice(b);
        }
        Cbor::Text(s) => {
            write_head(3, s.len() as u64, out);
            out.extend_from_slice(s.as_bytes());
        }
        Cbor::Array(a) => {
            write_head(4, a.len() as u64, out);
            for it in a {
                enc(it, out);
            }
        }
        Cbor::Map(pairs) => {
            // RFC 8949 §4.2: sort by bytewise lexicographic order of encoded keys.
            let mut encoded: Vec<(Vec<u8>, Vec<u8>)> = pairs
                .iter()
                .map(|(k, v)| {
                    let mut kb = Vec::new();
                    enc(k, &mut kb);
                    let mut vb = Vec::new();
                    enc(v, &mut vb);
                    (kb, vb)
                })
                .collect();
            encoded.sort_by(|a, b| a.0.cmp(&b.0));
            write_head(5, encoded.len() as u64, out);
            for (kb, vb) in encoded {
                out.extend_from_slice(&kb);
                out.extend_from_slice(&vb);
            }
        }
        Cbor::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        Cbor::Null => out.push(0xf6),
        Cbor::Tag(t, inner) => {
            write_head(6, *t, out);
            enc(inner, out);
        }
    }
}

/// Deterministically encode a value (RFC 8949 §4.2 core deterministic).
pub fn encode_canonical(c: &Cbor) -> Vec<u8> {
    let mut out = Vec::new();
    enc(c, &mut out);
    out
}

fn read_arg(b: &[u8], ai: u8, pos: &mut usize) -> Result<u64, String> {
    match ai {
        0..=23 => Ok(ai as u64),
        24 => {
            let v = *b.get(*pos).ok_or("eof u8")? as u64;
            *pos += 1;
            Ok(v)
        }
        25 => {
            let s = b.get(*pos..*pos + 2).ok_or("eof u16")?;
            *pos += 2;
            Ok(u16::from_be_bytes([s[0], s[1]]) as u64)
        }
        26 => {
            let s = b.get(*pos..*pos + 4).ok_or("eof u32")?;
            *pos += 4;
            Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as u64)
        }
        27 => {
            let s = b.get(*pos..*pos + 8).ok_or("eof u64")?;
            *pos += 8;
            let mut a = [0u8; 8];
            a.copy_from_slice(s);
            Ok(u64::from_be_bytes(a))
        }
        _ => Err("bad additional info".into()),
    }
}

fn dec_at(b: &[u8], pos: &mut usize) -> Result<Cbor, String> {
    let ib = *b.get(*pos).ok_or("eof head")?;
    *pos += 1;
    let major = ib >> 5;
    let ai = ib & 0x1f;
    match major {
        0 => Ok(Cbor::U(read_arg(b, ai, pos)?)),
        1 => Ok(Cbor::I(-1 - (read_arg(b, ai, pos)? as i64))),
        2 => {
            let n = read_arg(b, ai, pos)? as usize;
            let s = b.get(*pos..*pos + n).ok_or("eof bytes")?.to_vec();
            *pos += n;
            Ok(Cbor::Bytes(s))
        }
        3 => {
            let n = read_arg(b, ai, pos)? as usize;
            let s = b.get(*pos..*pos + n).ok_or("eof text")?;
            *pos += n;
            Ok(Cbor::Text(String::from_utf8(s.to_vec()).map_err(|_| "utf8")?))
        }
        4 => {
            let n = read_arg(b, ai, pos)?;
            let mut a = Vec::new();
            for _ in 0..n {
                a.push(dec_at(b, pos)?);
            }
            Ok(Cbor::Array(a))
        }
        5 => {
            let n = read_arg(b, ai, pos)?;
            let mut m = Vec::new();
            for _ in 0..n {
                let k = dec_at(b, pos)?;
                let v = dec_at(b, pos)?;
                m.push((k, v));
            }
            Ok(Cbor::Map(m))
        }
        6 => {
            let t = read_arg(b, ai, pos)?;
            Ok(Cbor::Tag(t, Box::new(dec_at(b, pos)?)))
        }
        7 => match ai {
            20 => Ok(Cbor::Bool(false)),
            21 => Ok(Cbor::Bool(true)),
            22 => Ok(Cbor::Null),
            _ => Err("unsupported simple".into()),
        },
        _ => Err("unknown major".into()),
    }
}

/// Decode a single CBOR item; returns the value and number of bytes consumed.
pub fn decode(b: &[u8]) -> Result<(Cbor, usize), String> {
    let mut pos = 0usize;
    let v = dec_at(b, &mut pos)?;
    Ok((v, pos))
}

// ─────────────────────────── Signing pre-image ───────────────────────────

/// Build the transport-independent signing pre-image:
/// `DOMAIN_TAG || selector || detCBOR(body \ "signature")`.
pub fn preimage(t: MsgType, body: &Cbor) -> Vec<u8> {
    let stripped = body.without_key("signature");
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(DOMAIN_TAG);
    out.push(t.selector());
    out.extend_from_slice(&encode_canonical(&stripped));
    out
}

/// Sign a CRCP body. Returns the raw 64-byte ed25519 signature.
pub fn sign(sk: &SigningKey, t: MsgType, body: &Cbor) -> [u8; 64] {
    sk.sign(&preimage(t, body)).to_bytes()
}

/// Verify a CRCP body against a 64-byte signature.
pub fn verify(vk: &VerifyingKey, t: MsgType, body: &Cbor, sig: &[u8; 64]) -> bool {
    let sig = Signature::from_bytes(sig);
    vk.verify(&preimage(t, body), &sig).is_ok()
}

// ─────────────────────────── 0x04 payload unit ───────────────────────────

fn header_byte(t: MsgType, frag: bool) -> u8 {
    (CRCP_VERSION << 4) | ((frag as u8) << 3) | t.selector()
}

/// Encode a single (non-fragmented) CRCP payload unit: header byte + canonical CBOR body.
/// This is the application payload that rides inside an RFC 0001 `0x04` frame.
pub fn encode_wire(t: MsgType, body: &Cbor) -> Vec<u8> {
    let mut out = vec![header_byte(t, false)];
    out.extend_from_slice(&encode_canonical(body));
    out
}

/// Parsed header of a CRCP payload unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireHeader {
    pub version: u8,
    pub frag: bool,
    pub msg_type: MsgType,
}

/// Decode a non-fragmented payload unit into (header, body).
pub fn decode_wire(bytes: &[u8]) -> Result<(WireHeader, Cbor), String> {
    let hb = *bytes.first().ok_or("empty")?;
    let version = hb >> 4;
    let frag = (hb >> 3) & 1 == 1;
    let msg_type = MsgType::from_selector(hb & 0x3).ok_or("bad selector")?;
    if frag {
        return Err("fragmented unit: use Reassembler".into());
    }
    let (body, _) = decode(&bytes[1..])?;
    Ok((WireHeader { version, frag, msg_type }, body))
}

// ─────────────────────────── Fragmentation ───────────────────────────

/// Split a payload unit into fragments when the CBOR body exceeds `max`.
/// Each fragment = header byte (FRAG=1) + [frag_id, index, total] + body chunk.
/// Returns a single non-fragmented unit if it fits.
pub fn fragment_unit(t: MsgType, body: &Cbor, max: usize, frag_id: u8) -> Vec<Vec<u8>> {
    let body_bytes = encode_canonical(body);
    if body_bytes.len() <= max {
        return vec![encode_wire(t, body)];
    }
    let chunks: Vec<&[u8]> = body_bytes.chunks(max).collect();
    let total = chunks.len() as u8;
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut out = vec![header_byte(t, true), frag_id, i as u8, total];
            out.extend_from_slice(chunk);
            out
        })
        .collect()
}

/// Reassembles fragmented payload units keyed by `frag_id`.
#[derive(Default)]
pub struct Reassembler {
    parts: Vec<(u8, u8, Vec<u8>)>, // (frag_id, index, chunk)
    total: Option<u8>,
    msg_type: Option<MsgType>,
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one fragment. Returns Some((type, body)) once all fragments arrived.
    pub fn push(&mut self, fragment: &[u8]) -> Result<Option<(MsgType, Cbor)>, String> {
        let hb = *fragment.first().ok_or("empty frag")?;
        let frag = (hb >> 3) & 1 == 1;
        let mt = MsgType::from_selector(hb & 0x3).ok_or("bad selector")?;
        if !frag {
            // Not fragmented — decode directly.
            let (_, body) = decode_wire(fragment)?;
            return Ok(Some((mt, body)));
        }
        let frag_id = *fragment.get(1).ok_or("eof frag_id")?;
        let index = *fragment.get(2).ok_or("eof index")?;
        let total = *fragment.get(3).ok_or("eof total")?;
        let chunk = fragment.get(4..).ok_or("eof chunk")?.to_vec();
        self.total = Some(total);
        self.msg_type = Some(mt);
        if !self.parts.iter().any(|(fid, idx, _)| *fid == frag_id && *idx == index) {
            self.parts.push((frag_id, index, chunk));
        }
        if self.parts.len() as u8 == total {
            let mut ordered = self.parts.clone();
            ordered.sort_by_key(|(_, idx, _)| *idx);
            let mut body_bytes = Vec::new();
            for (_, _, c) in ordered {
                body_bytes.extend_from_slice(&c);
            }
            let (body, _) = decode(&body_bytes)?;
            return Ok(Some((mt, body)));
        }
        Ok(None)
    }
}

/// True if an e-stop body fits in a single packet (RFC 0003 §4: e-stop MUST NOT fragment).
pub fn estop_fits_single_packet(body: &Cbor) -> bool {
    encode_canonical(body).len() <= MAX_CRCP_FRAGMENT
}

// ─────────────────────────────── Tests ───────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task(sig: Option<Vec<u8>>) -> Cbor {
        let mut pairs = vec![
            (Cbor::text("protocol"), Cbor::text("crcp/1")),
            (Cbor::text("task_id"), Cbor::Bytes(vec![1u8; 16])),
            (Cbor::text("agent_id"), Cbor::Bytes(vec![2u8; 16])),
            (Cbor::text("intent"), Cbor::text("physical.kitchen.fetch")),
            (Cbor::text("deadman_ms"), Cbor::U(3000)),
            (Cbor::text("idempotency_key"), Cbor::text("abc12345")),
            (Cbor::text("issued_at"), Cbor::Tag(1, Box::new(Cbor::U(1_900_000_000)))),
        ];
        if let Some(s) = sig {
            pairs.push((Cbor::text("signature"), Cbor::Bytes(s)));
        }
        Cbor::Map(pairs)
    }

    #[test]
    fn canonical_is_order_independent() {
        let a = Cbor::Map(vec![
            (Cbor::text("b"), Cbor::U(2)),
            (Cbor::text("a"), Cbor::U(1)),
        ]);
        let b = Cbor::Map(vec![
            (Cbor::text("a"), Cbor::U(1)),
            (Cbor::text("b"), Cbor::U(2)),
        ]);
        assert_eq!(encode_canonical(&a), encode_canonical(&b));
    }

    #[test]
    fn shortest_int_form() {
        assert_eq!(encode_canonical(&Cbor::U(10)), vec![0x0a]);
        assert_eq!(encode_canonical(&Cbor::U(200)), vec![0x18, 0xc8]);
        assert_eq!(encode_canonical(&Cbor::U(300)), vec![0x19, 0x01, 0x2c]);
    }

    #[test]
    fn roundtrip_decode() {
        let v = sample_task(Some(vec![9u8; 64]));
        let bytes = encode_canonical(&v);
        let (dec, used) = decode(&bytes).unwrap();
        assert_eq!(used, bytes.len());
        // Re-encoding the decoded value is canonical and stable.
        assert_eq!(encode_canonical(&dec), bytes);
    }

    #[test]
    fn preimage_excludes_signature() {
        let with = sample_task(Some(vec![7u8; 64]));
        let without = sample_task(None);
        assert_eq!(preimage(MsgType::Task, &with), preimage(MsgType::Task, &without));
    }

    #[test]
    fn preimage_has_domain_and_selector() {
        let body = sample_task(None);
        let pi = preimage(MsgType::Task, &body);
        assert_eq!(&pi[..5], DOMAIN_TAG);
        assert_eq!(pi[5], MsgType::Task.selector());
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let sk = SigningKey::from_bytes(&[42u8; 32]);
        let vk = sk.verifying_key();
        let body = sample_task(None);
        let sig = sign(&sk, MsgType::Task, &body);
        assert!(verify(&vk, MsgType::Task, &body, &sig));
        // Signature is transport-independent: same sig verifies on a body that
        // additionally carries the signature field (as on the wire).
        let mut on_wire = sample_task(Some(sig.to_vec()));
        assert!(verify(&vk, MsgType::Task, &on_wire, &sig));
        // Tamper the body -> verify fails.
        if let Cbor::Map(p) = &mut on_wire {
            p.push((Cbor::text("intent_tampered"), Cbor::Bool(true)));
        }
        assert!(!verify(&vk, MsgType::Task, &on_wire, &sig));
    }

    #[test]
    fn wrong_type_selector_breaks_signature() {
        let sk = SigningKey::from_bytes(&[1u8; 32]);
        let vk = sk.verifying_key();
        let body = sample_task(None);
        let sig = sign(&sk, MsgType::Task, &body);
        // Same body, different message type -> different pre-image -> invalid.
        assert!(!verify(&vk, MsgType::Manifest, &body, &sig));
    }

    #[test]
    fn wire_header_roundtrip() {
        let body = sample_task(Some(vec![3u8; 64]));
        let unit = encode_wire(MsgType::Telemetry, &body);
        let (hdr, decoded) = decode_wire(&unit).unwrap();
        assert_eq!(hdr.version, CRCP_VERSION);
        assert!(!hdr.frag);
        assert_eq!(hdr.msg_type, MsgType::Telemetry);
        assert_eq!(encode_canonical(&decoded), encode_canonical(&body));
    }

    #[test]
    fn fragmentation_roundtrip() {
        // Force a large body well over MAX_CRCP_FRAGMENT.
        let big = Cbor::Map(vec![
            (Cbor::text("protocol"), Cbor::text("crcp/1")),
            (Cbor::text("blob"), Cbor::Bytes(vec![0xABu8; 600])),
        ]);
        let frags = fragment_unit(MsgType::Task, &big, MAX_CRCP_FRAGMENT, 77);
        assert!(frags.len() > 1, "expected multiple fragments");
        let mut re = Reassembler::new();
        let mut done = None;
        // Feed out of order to prove reordering works.
        for f in frags.iter().rev() {
            if let Some(x) = re.push(f).unwrap() {
                done = Some(x);
            }
        }
        let (mt, body) = done.expect("reassembled");
        assert_eq!(mt, MsgType::Task);
        assert_eq!(encode_canonical(&body), encode_canonical(&big));
    }

    #[test]
    fn small_unit_not_fragmented() {
        // A signed TaskEnvelope (64B sig + two 16B ids) exceeds 200B and legitimately
        // fragments (see fragmentation_roundtrip); use a genuinely small body here.
        let small = Cbor::Map(vec![
            (Cbor::text("protocol"), Cbor::text("crcp/1")),
            (Cbor::text("status"), Cbor::text("ok")),
        ]);
        let frags = fragment_unit(MsgType::Task, &small, MAX_CRCP_FRAGMENT, 1);
        assert_eq!(frags.len(), 1);
    }

    #[test]
    fn estop_fits_one_packet() {
        let estop = Cbor::Map(vec![
            (Cbor::text("protocol"), Cbor::text("crcp/1")),
            (Cbor::text("kind"), Cbor::text("estop")),
            (Cbor::text("agent_id"), Cbor::Bytes(vec![1u8; 16])),
            (Cbor::text("reason"), Cbor::text("operator")),
            (Cbor::text("nonce"), Cbor::Bytes(vec![9u8; 16])),
            (Cbor::text("signature"), Cbor::Bytes(vec![0u8; 64])),
            (Cbor::text("ts"), Cbor::Tag(1, Box::new(Cbor::U(1_900_000_000)))),
        ]);
        assert!(estop_fits_single_packet(&estop));
    }
}
