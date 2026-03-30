/// Result of a drive attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveOutcome {
  /// Progress was made.
  Progressed,
  /// No progress was made.
  Idle,
}
