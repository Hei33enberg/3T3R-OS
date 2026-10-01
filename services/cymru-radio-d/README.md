# cymru-radio-d

Rust daemon that drives the LoRa/HF radio modem and exposes a D-Bus IPC interface to userspace apps (the 3T3R app, the agent runtime, `@mosadd/mcp`).

> Part of the experimental radio-node prototype — not the 33.0S frame and not a product. The crate keeps its legacy `cymru` slug as a build identifier.

## Responsibilities

1. **SPI control** of the SX1262 LoRa modem via `/dev/spidev0.0` (no kernel driver — userspace LoRa is cleaner)
2. **HF modem control** via USB ACM if hardware present (RTL-SDR or dedicated HF chip)
3. **Char device** `/dev/cymru-radio` for low-level packet I/O (debug + advanced use)
4. **D-Bus interface** `org.cymru.Radio` on the system bus (see [`docs/architecture/`](../../docs/architecture/README.md) and [RFC 0002](../../docs/rfcs/0002-dbus-ipc.md))
5. **Carrier multiplexing** — TDM between LoRa 868, LoRa 915, HF
6. **Forward error correction** — Reed-Solomon RS(255, 223) / RS(255, 191)

## Architecture

```
                    apps (opt-in)
                          │
                          ▼  D-Bus
                  ┌────────────────┐
                  │  cymru-radio-d │
                  │   (this crate) │
                  └────────────────┘
                          │
                ┌─────────┴─────────┐
                ▼                   ▼
        SPI: /dev/spidev0.0    USB ACM: /dev/ttyACM0
        (SX1262 LoRa)          (HF modem, if present)
```

## Crates

```
embedded-hal         = "1"
linux-embedded-hal   = "0.4"
spidev               = "0.6"
tokio                = { version = "1", features = ["full"] }
zbus                 = "4"  # D-Bus
serde                = { version = "1", features = ["derive"] }
tracing              = "0.1"
reed-solomon-erasure = "6"
```

## Status

Measured from the code in this repository (01.10.2026); no dates are promised.

- Built and tested without hardware: frame encode/decode (RFC 0001), Reed-Solomon error correction, KISS framing, the priority transmit queue and a mock radio backend, with unit tests.
- Not built yet: the SX1262 SPI driver, the D-Bus interface `org.cymru.Radio` (RFC 0002) and multi-carrier operation on real hardware.

## License

Apache-2.0 (parent repo LICENSE).
