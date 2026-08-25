# 3T3R OS Architecture

## Layered design

```
┌─────────────────────────────────────────────────┐
│ Layer 7: Apps (release artifacts, NOT source)   │
│   cymru-main PWA bundle                         │
│   cymru-agent Python package                    │
│   @m0ssad/mcp npm package                       │
├─────────────────────────────────────────────────┤
│ Layer 6: cymru-os system services (this repo)   │
│   cymru-radio-d (Rust) — LoRa/HF modem driver  │
│   cymru-bridge-d — D-Bus IPC                   │
│   cymru-mesh-d — Reticulum routing             │
│   cymru-otad — RAUC OTA agent                  │
├─────────────────────────────────────────────────┤
│ Layer 5: Runtime libs                           │
│   Node.js 20, Python 3.11                      │
│   PipeWire (audio), Cog (kiosk WebKit)         │
├─────────────────────────────────────────────────┤
│ Layer 4: DSP libs                               │
│   libcodec2, libopus, GNU Radio                │
│   Reticulum (Python)                            │
├─────────────────────────────────────────────────┤
│ Layer 3: Kernel drivers                         │
│   SPI (SX1262 LoRa), USB ACM (HF SDR)          │
│   GPIO (PTT button)                              │
├─────────────────────────────────────────────────┤
│ Layer 2: Linux mainline 6.6 LTS + RPi patches  │
├─────────────────────────────────────────────────┤
│ Layer 1: U-Boot                                 │
├─────────────────────────────────────────────────┤
│ Layer 0: RPi CM5 + custom carrier PCB          │
└─────────────────────────────────────────────────┘
```

## Critical principle

3T3R OS **does not modify** the 3T3R app or mosADD OS source code. We pull **release artifacts** (latest stable from GitHub Releases / npm) and install them as systemd units.

The apps don't know they're running on 3T3R RAYDIO — unless they opt-in by calling the D-Bus IPC bridge `org.cymru.Radio`.

## D-Bus IPC contract

System bus interface: `org.cymru.Radio`

Object path: `/org/cymru/Radio`

### Methods

```
SendMessage(string recipient, string payload) -> string message_id
BroadcastMessage(string channel, string payload) -> string message_id
Subscribe(string channel) -> object_path subscription
Unsubscribe(object_path subscription)
GetCarrierAvailability() -> dict<string, bool>
    # Returns: {"lora_868": true, "lora_915": false, "hf_codec2": true}
```

### Signals

```
MessageReceived(string sender, string channel, string payload)
CarrierChanged(string carrier_name, bool available)
```

### Authorization

D-Bus PolicyKit rule: apps must declare `org.cymru.Radio.Use` permission in their `.policy` file. Default: deny. User opt-in via systems settings menu.

## Frame format (radio wire)

```
| 0xC9 0xB0 | length (u16 BE) | KISS payload |
   magic       length field      KISS-encoded packet
```

KISS payload contains:
- Type byte: 0x01 = data, 0x02 = control, 0x03 = voice (Codec2 1300)
- Sender ID (16 bytes)
- Recipient ID (16 bytes) or 0xFF... for broadcast
- Application payload

Forward error correction: Reed-Solomon RS(255, 223) on data type, RS(255, 191) on voice (more redundancy).

## Carrier multiplexing

`cymru-radio-d` rotates between carriers (LoRa 868, LoRa 915, HF Codec2 if hardware) using TDM (Time Division Multiplexing) with 100ms slots. Apps don't choose carrier directly — `cymru-radio-d` picks based on:

1. Recipient's last-known reachable carrier (cached)
2. Carrier availability (regulatory + RSSI)
3. Cost (HF is "free" airtime, LoRa has duty cycle constraints under EN 300 220)

Tool capability flags from `@m0ssad/mcp` (`requires: "network" | "radio" | "any"`) filter which apps can use which transport.
