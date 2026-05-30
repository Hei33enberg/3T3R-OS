# RFC 0001 — Radio frame format

**Status:** draft
**Authors:** Major Boga (Agent CYMRU)
**Target release:** v0.1.0 (with cymru-radio-d C3)

## Context

CYMRU RAYDIO devices communicate over LoRa (ISM 868/915 MHz) and optional HF
radio. The wire format must be:

1. **Magic-byte distinguishable** from any other LoRa/HF protocol so receivers
   can drop foreign packets cheaply at the modem layer
2. **Length-prefixed** so the receiver can frame without ambiguity
3. **FEC-protected** because LoRa link is lossy and HF is lossier still
4. **Encryption-agnostic** at this layer — keys come from `@m0ssad/crypto`'s
   Double Ratchet on the app side, and from cymru-main's voice biometric on
   the device side. cymru-radio-d does not decrypt.

## Frame format

```
Offset   Size   Field
-------- ------ -----------------------------------------------------
0        2      Magic: 0xC9 0xB0 ("CYMRU" → 'C'=0xC9 'B'(óg)=0xB0)
2        2      Length (u16 big-endian) — bytes 4..end (excludes header)
4        N      KISS-encoded payload (KISS frame end-byte = 0xC0)
```

### Magic byte rationale

`0xC9 0xB0` chosen because:
- High bits set on both bytes → unlikely false positive on noise/null fill
- Not collide with KISS frame escapes (0xC0, 0xDB) or AX.25 flag (0x7E)
- Mnemonic "C" + "Bóg"

### KISS payload

KISS (Keep It Simple, Stupid) framing wraps the application payload:

```
Type byte:
  0x01 = data (Reed-Solomon RS(255, 223))
  0x02 = control (Reed-Solomon RS(255, 223))
  0x03 = voice (Codec2 1300 bps, Reed-Solomon RS(255, 191))
  0x04 = robot-dispatch (CRCP, deterministic CBOR; FEC by safety_class — see RFC 0003)

Sender ID:    16 bytes (random per-device, persistent in /etc/cymru/identity)
Recipient ID: 16 bytes (or 0xFF * 16 for broadcast)
Channel ID:   4 bytes (mIRC channel hash or 0x00000000 for mDM)
Application payload: variable
```

The application payload is opaque to cymru-radio-d. cymru-main sees PTT voice
turns; cymru-agent sees skill-dispatch envelopes; mosadd-mcp sees mDM
messages.

## FEC choice

Reed-Solomon erasure coding via `reed-solomon-erasure` crate. Two profiles:

- **RS(255, 223)** for data and control — 12% overhead, can correct up to
  16 byte errors per 255-byte block. Suitable for LoRa BW 125 kHz SF7-9 at
  range up to ~5 km.
- **RS(255, 191)** for voice — 25% overhead, can correct up to 32 byte
  errors. Suitable for HF Codec2 at range up to ~1000+ km but with
  significant fading.

## Carrier hint

Top 4 bits of the channel ID's first byte encode a carrier preference hint
that `cymru-radio-d` uses when multiplexing:

- `0x0_` — any carrier
- `0x1_` — prefer LoRa 868 MHz
- `0x2_` — prefer LoRa 915 MHz (US/Brazil/Australia)
- `0x3_` — prefer HF Codec2 (long-range)
- `0xF_` — broadcast (any carrier)

This is a HINT, not a constraint. cymru-radio-d may pick a different carrier
if the hinted one is unavailable.

## Open questions

- [ ] Reed-Solomon block boundary handling for payloads > 223 bytes (CRCP 0x04 fragmentation defined in RFC 0003 §4; generalize?)
- [ ] Bluetooth LE as additional carrier for short-range device-to-device
- [ ] M17 protocol compatibility (M17 is a competing open ham radio protocol)
- [ ] Backoff and ARQ at this layer or at app layer?

## References

- KISS protocol: http://www.ax25.net/kiss.aspx
- LoRa modulation: Semtech SX1262 datasheet
- Codec2 1300 bps: https://www.rowetel.com/?page_id=452
- Reed-Solomon erasure: RFC 8489 §6.4
