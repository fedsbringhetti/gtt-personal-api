//! GTT's own open data: real-time trip updates for urban runs (Phase 3, decision N).
//!
//! The feeds are plain HTTP GETs returning GTFS Realtime messages (Protocol Buffers). Every
//! response is generated on request, so it differs byte by byte even when nothing changed:
//! freshness comes from each run's own timestamp, never from the header.

pub mod comparison;
#[rustfmt::skip]
#[allow(clippy::all, clippy::pedantic, rustdoc::all, missing_docs)]
pub mod proto;
pub mod trip_updates;
