//! Flight recorder modules.

/// Remoting flight recorder for diagnostics.
pub mod remoting_flight_recorder;
/// Public recorder type alias.
pub type RemotingFlightRecorder = remoting_flight_recorder::RemotingFlightRecorder;
/// Flight recorder snapshot type alias.
pub(crate) type RemotingFlightRecorderSnapshot = remoting_flight_recorder::RemotingFlightRecorderSnapshot;
