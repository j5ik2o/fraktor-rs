//! Protocol commands for the backoff supervisor actor.

#[cfg(test)]
mod tests;

/// Protocol commands for the backoff supervisor actor.
///
/// Corresponds to Pekko's `BackoffSupervisor` companion object messages
/// (`GetCurrentChild`, `Reset`, `GetRestartCount`).
#[derive(Clone, Debug)]
pub enum BackoffSupervisorCommand {
  /// Query the current child actor.
  GetCurrentChild,
  /// Reset the backoff restart counter.
  Reset,
  /// Query the current restart count.
  GetRestartCount,
}
