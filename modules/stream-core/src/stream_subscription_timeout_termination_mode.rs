#[cfg(test)]
mod tests;

/// Termination action when a stream subscription timeout fires.
///
/// Mirrors Pekko's `StreamSubscriptionTimeoutTerminationMode` sealed
/// hierarchy (`NoopTermination` / `WarnTermination` / `CancelTermination`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSubscriptionTimeoutTerminationMode {
  /// Do nothing when the subscription timeout fires.
  Noop,
  /// Emit a warning when the subscription timeout fires.
  Warn,
  /// Cancel the publisher when the subscription timeout fires.
  Cancel,
}
