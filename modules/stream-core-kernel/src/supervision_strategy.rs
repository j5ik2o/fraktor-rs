#[cfg(test)]
#[path = "supervision_strategy_test.rs"]
mod tests;

/// Supervision strategy deciding how a stage handles processing failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionStrategy {
  /// Stops the stream with the observed failure.
  Stop,
  /// Skips the failing element and continues.
  Resume,
  /// Restarts the stage state and continues.
  Restart,
}
