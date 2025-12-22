//! Stream drive outcome definitions.

/// Outcome of a drive cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveOutcome {
  /// Drive cycle made progress.
  Progressed,
  /// Drive cycle did not make progress.
  Idle,
}
