//! Cancellation strategy definitions for stream stages.

#[cfg(test)]
mod tests;

/// Strategy applied when a stage receives a cancellation signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationStrategyKind {
  /// Complete the stage normally on cancellation.
  CompleteStage,
  /// Fail the stage on cancellation.
  FailStage,
  /// Propagate the failure upstream.
  PropagateFailure,
}
