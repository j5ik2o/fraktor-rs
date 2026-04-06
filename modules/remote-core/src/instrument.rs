//! Instrumentation hooks and in-memory flight recorder for the remote
//! subsystem.
//!
//! This module intentionally depends only on `alloc` and the shared types
//! from `actor-core`; it has no transport-specific features and no `tokio`
//! references (see `remote-core-instrument` spec).

#[cfg(test)]
mod tests;

mod flight_recorder;
mod flight_recorder_event;
mod flight_recorder_snapshot;
mod handshake_phase;
mod remote_instrument;

pub use flight_recorder::RemotingFlightRecorder;
pub use flight_recorder_event::FlightRecorderEvent;
pub use flight_recorder_snapshot::RemotingFlightRecorderSnapshot;
pub use handshake_phase::HandshakePhase;
pub use remote_instrument::RemoteInstrument;
