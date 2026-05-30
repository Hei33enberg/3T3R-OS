//! cymru-otad (C8) — RAUC OTA update agent (A/B partition swap, signed bundles,
//! rollback on boot failure). Skeleton.

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(
        "cymru-otad {} — skeleton; RAUC A/B OTA TODO (C8)",
        env!("CARGO_PKG_VERSION")
    );
}
