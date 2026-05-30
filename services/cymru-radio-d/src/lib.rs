//! cymru-radio-d core — RFC 0001 framing + TDM priority queue + mock radio.
//!
//! Platform-independent logic (frames, scheduling, carrier mux) lives here and is
//! unit-tested on any host. The SX1262 SPI backend is Linux-only and plugs in via
//! the [`Radio`] trait (see [`MockRadio`] for the cross-platform test/dev backend).

pub mod frame;
pub mod queue;
pub mod radio;

pub use frame::{Frame, BROADCAST, MAGIC};
pub use queue::{Priority, TxQueue};
pub use radio::{MockEther, MockRadio, Radio};
