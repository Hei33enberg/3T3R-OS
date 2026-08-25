# RFC 0002 — D-Bus IPC contract (`org.cymru.Radio` + `org.cymru.Mesh`)

**Status:** draft
**Authors:** Major Boga (3T3R / CTO)
**Target release:** v0.1.0 (with `cymru-bridge-d` C7), `org.cymru.Mesh` lands with `cymru-mesh-d` C9
**Supersedes:** the D-Bus sketch in [`docs/architecture/README.md`](../architecture/README.md) — that section is now informative; this RFC is normative.

## Context

3T3R RAYDIO's defining rule: **apps work standalone, with ZERO code dependency on 3T3R OS.** A cymru-main PWA bundle, a cymru-agent Python package and `@m0ssad/mcp` all run unmodified on the device exactly as they run on a phone or VPS.

Convergence happens **only** here, and it is **opt-in**: an app that wants off-grid radio/mesh transport asks for it over the system D-Bus bus. An app that never calls D-Bus never knows it's on hardware. This RFC is the contract between:

- **Producers** (cymru-os services): `cymru-bridge-d` (C7) owns the bus names and brokers; `cymru-radio-d` (C3) backs `org.cymru.Radio`; `cymru-mesh-d` (C9) backs `org.cymru.Mesh`.
- **Consumers** (release artifacts): cymru-main (PTT voice turns), cymru-agent (skill-dispatch envelopes), `@m0ssad/mcp` (mDM/mTALK frames).

The wire format below the bus is [RFC 0001](./0001-frame-format.md); this RFC never duplicates it — D-Bus payloads are opaque `ay` byte arrays that `cymru-radio-d` wraps into KISS frames.

## Bus topology

- **Bus:** system bus (`/run/dbus/system_bus_socket`). Not session bus — services are long-lived systemd units (see cymru-agent `packaging/systemd/cymru-agent.service`).
- **Well-known names:** `org.cymru.Radio`, `org.cymru.Mesh` (both owned by `cymru-bridge-d`, which proxies to `cymru-radio-d` / `cymru-mesh-d` over private peer sockets).
- **Object paths:** `/org/cymru/Radio`, `/org/cymru/Mesh`. Subscriptions get child paths `/org/cymru/Radio/sub/<u>`.

## Interface `org.cymru.Radio`

Direct radio transport (RFC 0001 frames over LoRa/HF). Connectionless, best-effort + FEC; no ordering guarantee across carriers.

### Methods

| Signature | Returns | Notes |
|-----------|---------|-------|
| `SendMessage(s recipient, ay payload)` | `s message_id` | `recipient` = hex device-id or `"broadcast"`. `payload` opaque, ≤ `MaxPayload`. |
| `BroadcastMessage(s channel, ay payload)` | `s message_id` | `channel` = mIRC-style channel name (hashed to RFC 0001 4-byte Channel ID). |
| `Subscribe(s channel)` | `o subscription` | Returns an object path; `MessageReceived` fires for matching frames until released. `""` = all channels addressed to this device. |
| `Unsubscribe(o subscription)` | — | Idempotent; unknown path is a no-op. |
| `GetCarrierAvailability()` | `a{sb}` | e.g. `{"lora_868": true, "lora_915": false, "hf_codec2": true}`. |

`recipient`/`channel` are **strings** at the bus boundary for ergonomics; `cymru-radio-d` deterministically maps them to RFC 0001 16-byte Sender/Recipient IDs and the 4-byte Channel ID (BLAKE3-128 truncation, documented in cymru-radio-d). Apps never see raw IDs.

`payload` is `ay` (byte array), **not** `s` — the architecture sketch used `string`, but app payloads are already-encrypted binary (`@m0ssad/crypto` Double Ratchet, cymru voice frames). Forcing UTF-8 would corrupt them.

### Signals

| Signature | Notes |
|-----------|-------|
| `MessageReceived(s sender, s channel, ay payload)` | Delivered only to clients holding a matching `Subscribe`. `channel` is `""` for direct (mDM) frames. |
| `CarrierChanged(s carrier, b available)` | Regulatory duty-cycle lockout, antenna unplug, RSSI floor, etc. |

### Properties (read-only)

| Name | Type | Notes |
|------|------|-------|
| `DeviceId` | `s` | This device's stable hex id (from `/etc/cymru/identity`, RFC 0001). |
| `Carriers` | `a{sb}` | Same shape as `GetCarrierAvailability()`; emits `PropertiesChanged`. |
| `MaxPayload` | `u` | Largest `payload` accepted before fragmentation is required (app-side concern). |

### Errors

- `org.cymru.Radio.Error.NotAuthorized` — caller lacks the PolicyKit action (see Authorization).
- `org.cymru.Radio.Error.NoCarrier` — no carrier currently available for the hinted/required band.
- `org.cymru.Radio.Error.PayloadTooLarge` — `payload` > `MaxPayload`.
- `org.cymru.Radio.Error.UnknownRecipient` — `recipient` not a valid device-id and not `"broadcast"`.

## Interface `org.cymru.Mesh`

Multi-hop store-and-forward over Reticulum/LXMF (`cymru-mesh-d`, C9). Uses `org.cymru.Radio` as one of its outgoing interfaces but adds routing + delivery receipts. Apps that only need point-to-point use `Radio`; apps that want network effects (relayed delivery beyond direct RF range) use `Mesh`.

### Methods

| Signature | Returns | Notes |
|-----------|---------|-------|
| `SendLxmf(s destination_hash, ay content)` | `s lxmf_id` | Reticulum destination hash (hex). Queued + retried per LXMF. |
| `Announce(s aspect)` | — | Announce this node's identity on `aspect` (e.g. `"cymru.god"`, `"mosadd.mdm"`). |
| `GetPath(s destination_hash)` | `(b u)` | `(reachable, hops)`; triggers path discovery if unknown. |

### Signals

| Signature | Notes |
|-----------|-------|
| `LxmfReceived(s source_hash, ay content)` | Inbound LXMF message. |
| `DeliveryReceipt(s lxmf_id, b delivered)` | Proof-of-delivery (LXMF). |
| `PathDiscovered(s destination_hash, u hops)` | Result of `GetPath` / passive announce learning. |

### Errors

- `org.cymru.Mesh.Error.NotAuthorized`, `org.cymru.Mesh.Error.NoPath`, `org.cymru.Mesh.Error.NotAnnounced`.

## Authorization (opt-in, default deny)

D-Bus access is gated by PolicyKit, **default deny**. Each app ships a `.policy` declaring the action it needs:

- `org.cymru.Radio.Use` — call any `org.cymru.Radio` method / receive its signals.
- `org.cymru.Mesh.Use` — same for `org.cymru.Mesh`.

The user grants/revokes per app in the 3T3R RAYDIO settings menu (cymru-main surfaces this). A fresh install grants nothing → apps behave exactly as off-device until the user opts in. `cymru-bridge-d` checks the action via `polkit` before brokering each first call from a connection and caches the verdict per-connection.

## Capability-flag bridge (coordination: mosadd-os D7 / [LINEAR-2362](https://linear.app/ip-ra/issue/LINEAR-2362))

`@m0ssad/mcp` tools declare `meta.requires: "network" | "radio" | "any"`. The host (`npx @m0ssad/mcp` under cymru-os, or cymru-agent) filters the advertised toolset by **live transport availability**:

| `requires` | Available iff | Routing |
|------------|---------------|---------|
| `network` | IP reachable (WiFi/eth/Starlink) | normal HTTPS — never touches this bus |
| `radio` | `GetCarrierAvailability()` has any `true` | `org.cymru.Radio` (direct) or `org.cymru.Mesh` (relayed) |
| `any` | either of the above | prefer IP, fall back to radio/mesh |

This is the **one place** the mosadd standalone contract and the cymru-os convergence layer touch — and it touches through a string enum and this bus, never through shared code. The `requires` enum string syntax is owned by mosadd-os (D7); this RFC consumes it. **Any change to the enum is a `requires:cymru-input` coordination event.**

## Introspection (codegen source of truth)

`cymru-bridge-d` (zbus, Rust) generates server stubs from, and apps introspect, the XML in [`docs/dbus-api/org.cymru.Radio.xml`](../dbus-api/) (to be added alongside C7 implementation). The XML is generated from this RFC, not hand-edited divergently — RFC is normative, XML is the machine-readable projection.

## Open questions

- [ ] Fragmentation: app-side (above the bus) or `cymru-radio-d`-side when `payload > MaxPayload`? Leaning app-side to keep the daemon stateless.
- [ ] Should `org.cymru.Mesh` expose `org.cymru.Radio` transparently (one interface, mesh-when-needed) instead of two? Keeping them split for v0.1 — explicit is debuggable.
- [ ] Per-connection vs per-call polkit check granularity under fast PTT bursts.
- [ ] Backpressure signal when a carrier's duty-cycle budget (EN 300 220) is exhausted mid-session.

## References

- [RFC 0001 — Radio frame format](./0001-frame-format.md)
- [architecture/README.md](../architecture/README.md) (informative D-Bus sketch, now superseded by this RFC)
- D-Bus specification — https://dbus.freedesktop.org/doc/dbus-specification.html
- Reticulum / LXMF — https://reticulum.network
- zbus (Rust D-Bus) — https://docs.rs/zbus
