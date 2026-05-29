<div align="center">

# cymru-os

**Linux distribution for CYMRU RAYDIO radio hardware**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-alpha-orange)](https://github.com/Hei33enberg/cymru-os)
[![Hardware](https://img.shields.io/badge/hardware-RPi%20CM5-c51a4a)](https://www.raspberrypi.com/products/compute-module-5/)

</div>

---

## What this is

`cymru-os` is the **convergence layer** for CYMRU RAYDIO hardware. It is the only place where three otherwise-independent products meet:

1. **cymru-main** — Personal God voice AI app (PWA from [github.com/Hei33enberg/CYMRU](https://github.com/Hei33enberg/CYMRU))
2. **cymru-agent** — Python Hermes orchestrator
3. **mosadd-os** — Apache 2.0 OSS communication suite (npm `@m0ssad/mcp`)

cymru-os pulls each as a **release artifact** (not source), installs them as systemd units, and exposes an optional D-Bus IPC bridge for cross-app coordination via the radio modem.

**The architectural rule (immutable, CEO 2026-05-29):** cymru-main and mosadd-os have ZERO code dependency. Convergence happens ONLY here, on hardware.

## Why federate

The three apps each have:
- Independent brand identity
- Independent user base
- Independent app store distribution
- Independent monetization

The CYMRU RAYDIO hardware is the moat that combines them at the OS level. When Apple/Samsung/Xiaomi wake up to where the meta is, they see two standalone brands with millions of users each, plus a piece of hardware they cannot retroactively decompose.

See [`docs/architecture/`](./docs/architecture/) for the layered architecture.

## What's in this repo

| Path | What |
|------|------|
| `configs/cymru_rpi_cm5_defconfig` | Buildroot config for RPi CM5 SD card image |
| `board/cymru-rpi-cm5/` | Board overlay (filesystem patches, systemd units, udev rules) |
| `services/cymru-radio-d/` | Rust daemon — LoRa/HF modem driver, exposes `/dev/cymru-radio` |
| `services/cymru-bridge-d/` | D-Bus IPC bridge between apps (Rust) |
| `services/cymru-mesh-d/` | Reticulum mesh stack wrapper |
| `services/cymru-otad/` | RAUC OTA update agent |
| `packages/cymru-main-pwa/` | Buildroot package that pulls cymru-main release |
| `packages/libcodec2/` | Codec2 1300 bps voice codec for radio |
| `packages/reticulum/` | Reticulum mesh routing |
| `docs/architecture/` | Layered architecture, design docs |
| `docs/rfcs/` | RFCs (e.g. radio protocol frame format) |
| `tests/integration/` | Two-RPi integration tests |
| `.github/workflows/` | CI: build sdcard.img, RAUC bundle, integration tests |

## Hardware target (v1)

- **SoC**: Raspberry Pi Compute Module 5 (BCM2712)
- **Radio (ISM)**: SX1262 LoRa modem, 868/915 MHz
- **Radio (HF, premium)**: Codec2-capable HF modem (TBD: SA868 or RTL-SDR USB)
- **Audio**: WM8731 codec via SPI
- **Display**: 128×128 LCD (MVP) → 1334×750 IPS (v0.3 Apple-bait form factor)
- **Controls**: 6-rotary encoders (3 CYMRU + 3 mosadd channels) + PTT button
- **Battery**: 3000mAh LiPo + BQ24295 charge IC

## Quickstart (when scaffolding lands)

```bash
# Clone + submodules (buildroot is huge — fetch shallow)
git clone https://github.com/Hei33enberg/cymru-os.git
cd cymru-os
git submodule update --init --depth=1 buildroot

# Configure for RPi CM5
make cymru_rpi_cm5_defconfig

# Build (takes 1-3 hours first time)
make -j$(nproc)

# Flash to SD card (output: output/images/sdcard.img)
sudo dd if=output/images/sdcard.img of=/dev/sdX bs=4M status=progress
```

## Status

- [ ] **C1** Buildroot config + boots to prompt on CM5 (target: 2026-06-30)
- [ ] **C2** Kernel drivers — SX1262 SPI + ALSA codec audio (target: 2026-07-15)
- [ ] **C3** `cymru-radio-d` Rust daemon (target: 2026-07-31)
- [ ] **C4** Two-RPi LoRa integration test (target: 2026-07-31)
- [ ] **C5** Kiosk shell — Cog WebKit boots cymru-main PWA (target: 2026-08-15)
- [ ] **C6** systemd units for all three apps (target: 2026-09-15)
- [ ] **C7** D-Bus IPC bridge `org.cymru.Radio` (target: 2026-10-15)
- [ ] **C8** OTA updater (RAUC) (target: 2026-11-30)
- [ ] **C9** Reticulum mesh integration (target: 2026-11-30)
- [ ] **C10** Triangulation service (target: 2026-12-31)

Full plan + handoff brief: [github.com/Hei33enberg/CYMRU/blob/main/docs/handoffs/firmware.md](https://github.com/Hei33enberg/CYMRU/blob/main/docs/handoffs/firmware.md)

Linear: [LINEAR-2341](https://linear.app/ip-ra/issue/LINEAR-2341) (Stream C sub-epic) under [LINEAR-2338](https://linear.app/ip-ra/issue/LINEAR-2338) (CYMRU RAYDIO master).

## License

Apache-2.0. See [LICENSE](./LICENSE).

**Why Apache 2.0 (not GPL)?** Patent-friendly (critical for radio/hardware where SX1262, Codec2, M17 patents have role), consistency with mosadd-os, no GPL infection risk for bundling proprietary cymru-main, future-licensing-friendly for 3rd party radio manufacturers (Midland, Motorola, Huawei licensing path open).

Note: kernel patches (when needed) carry their own GPL-2.0 license per Linux convention. Buildroot defconfig and our own services (cymru-radio-d, cymru-bridge-d, cymru-mesh-d, cymru-otad) are Apache 2.0.

## Related repos

- [Hei33enberg/CYMRU](https://github.com/Hei33enberg/CYMRU) — cymru-main (PWA/APK/Electron)
- [Hei33enberg/mosadd-os](https://github.com/Hei33enberg/mosadd-os) — mosadd Apache 2.0 OSS
- [Hei33enberg/cymru-hardware](https://github.com/Hei33enberg/cymru-hardware) — PCB schematics + BOM (TBD)
- [Hei33enberg/company.cymru](https://github.com/Hei33enberg/company.cymru) — LP + ecommerce (TBD)

## Contributing

This is part of the [CYMRU RAYDIO master plan](https://linear.app/ip-ra/issue/LINEAR-2338). External contributions welcome once C1 lands. For now this is a small-team build.

---

🫡 **Amen.**
