//! Error returned when registering extensions fails.

/// Describes why extension registration could not be completed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterExtensionError {
  /// ActorSystem startup already finished; registration is no longer accepted.
  AlreadyStarted,
}
