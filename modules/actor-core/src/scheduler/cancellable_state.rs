/// Enumerates the lifecycle of a scheduled job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellableState {
  /// Job has been allocated but not yet enqueued.
  Pending   = 0,
  /// Job is waiting in the timer wheel.
  Scheduled = 1,
  /// Job is currently executing.
  Executing = 2,
  /// Job completed successfully.
  Completed = 3,
  /// Job was cancelled before completion.
  Cancelled = 4,
}

impl From<u8> for CancellableState {
  fn from(value: u8) -> Self {
    match value {
      | 0 => Self::Pending,
      | 1 => Self::Scheduled,
      | 2 => Self::Executing,
      | 3 => Self::Completed,
      | _ => Self::Cancelled,
    }
  }
}
