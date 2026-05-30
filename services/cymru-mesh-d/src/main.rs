//! cymru-mesh-d (C9 / LINEAR-2539) — wraps Reticulum/LXMF for transport path A
//! (mDM/mTALK + logical agent dispatch). Kept as a SEPARATE daemon over IPC so the
//! restrictive-licensed Reticulum stack is never linked into proprietary/Apache code
//! (see docs/handoffs/raydio-transport-decision.md). Robot actuation/e-stop is path B
//! (cymru-radio-d), NOT here.

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("cymru-mesh-d {} — skeleton; Reticulum/LXMF wrapper TODO (C9)", env!("CARGO_PKG_VERSION"));
}
