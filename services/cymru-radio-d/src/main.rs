//! cymru-radio-d — LoRa/HF radio modem daemon for CYMRU OS.
//!
//! Drives the SX1262 LoRa modem via /dev/spidev0.0, optionally bridges an
//! HF modem via /dev/ttyACM0, and exposes the `org.cymru.Radio` D-Bus
//! interface for userspace apps (cymru-main, cymru-agent, mosadd-mcp).
//!
//! Status: skeleton — milestone C3 (target 2026-07-31). This main loop does
//! not yet talk to any hardware. The intent here is to get the binary, CLI,
//! tracing, and D-Bus service skeleton in place so subsequent commits can
//! drop in the SPI/HF driver code without architectural debate.
//!
//! See docs/architecture/README.md for the layered design and frame format.

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "cymru-radio-d", version, about = "CYMRU OS radio modem daemon")]
struct Args {
    /// SPI device path (LoRa SX1262)
    #[arg(long, default_value = "/dev/spidev0.0")]
    spi: String,

    /// Optional USB ACM device for HF modem
    #[arg(long)]
    hf_serial: Option<String>,

    /// D-Bus bus to register on
    #[arg(long, value_enum, default_value_t = BusKind::System)]
    bus: BusKind,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum BusKind {
    System,
    Session,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    info!(?args, "cymru-radio-d starting");

    // TODO(C3): open SPI device, init SX1262, start RX loop.
    // TODO(C3): expose `org.cymru.Radio` zbus interface.
    // TODO(C9): carrier multiplex (LoRa 868 | LoRa 915 | HF).

    warn!("skeleton main loop — no hardware driver yet. Sleeping forever.");
    std::future::pending::<()>().await;
    Ok(())
}
