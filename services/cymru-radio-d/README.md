# cymru-radio-d

Rust daemon that drives the LoRa/HF radio modem and exposes a D-Bus IPC interface to userspace apps (cymru-main, cymru-agent, mosadd-mcp).

## Responsibilities

1. **SPI control** of the SX1262 LoRa modem via `/dev/spidev0.0` (no kernel driver — userspace LoRa is cleaner)
2. **HF modem control** via USB ACM if hardware present (RTL-SDR or dedicated HF chip)
3. **Char device** `/dev/cymru-radio` for low-level packet I/O (debug + advanced use)
4. **D-Bus interface** `org.cymru.Radio` on the system bus (see [`docs/architecture/`](../../docs/architecture/))
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

- [ ] **C3** Skeleton daemon — talks to SPI, prints incoming packets (target: 2026-07-31)
- [ ] **C4** Two-RPi LoRa integration test — RPi-A SendMessage → RPi-B MessageReceived (target: 2026-07-31)
- [ ] **C7** D-Bus interface complete (target: 2026-10-15)
- [ ] **C9** Carrier multiplexing implementation (target: 2026-11-30)

## License

Apache-2.0 (parent repo LICENSE).
