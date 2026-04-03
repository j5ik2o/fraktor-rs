//! Stop reasons for classic FSM termination callbacks.

use alloc::string::String;

/// Reason recorded when a classic FSM stops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsmReason {
  /// Normal stop initiated by the FSM itself.
  Normal,
  /// Shutdown triggered by runtime teardown.
  Shutdown,
  /// Failure reason propagated from user code or runtime components.
  Failure(String),
}
