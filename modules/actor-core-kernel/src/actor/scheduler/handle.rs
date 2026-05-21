//! Handle returned when scheduling jobs.

#[cfg(test)]
#[path = "handle_test.rs"]
mod tests;

use fraktor_utils_core_rs::sync::ArcShared;

use super::cancellable::CancellableEntry;

/// Identifier for scheduled jobs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerHandle {
  raw:   u64,
  entry: ArcShared<CancellableEntry>,
}

impl SchedulerHandle {
  /// Creates a new handle from raw identifier.
  #[must_use]
  pub fn new(raw: u64) -> Self {
    Self { raw, entry: ArcShared::new(CancellableEntry::new()) }
  }

  /// Returns the inner identifier.
  #[must_use]
  pub const fn raw(&self) -> u64 {
    self.raw
  }

  /// Returns whether the job has been cancelled.
  ///
  /// Corresponds to Pekko's `Cancellable.isCancelled`.
  #[must_use]
  pub fn is_cancelled(&self) -> bool {
    self.entry.is_cancelled()
  }

  /// Returns whether the job has completed execution.
  #[must_use]
  pub fn is_completed(&self) -> bool {
    self.entry.is_completed()
  }

  /// Attempts to cancel the scheduled job.
  ///
  /// Returns `true` when the entry is in the `Scheduled` or `Executing` state
  /// and the cancellation transition succeeds. Returns `false` in all other
  /// cases: `Pending` (not yet scheduled), or already terminal
  /// (`Cancelled` or `Completed`).
  ///
  /// Corresponds to Pekko's `Cancellable.cancel`.
  ///
  /// Note: this marks the entry as cancelled but does not directly remove it
  /// from the scheduler's job queue. A scheduled entry is skipped on the next
  /// tick; an executing periodic entry is not rescheduled after the current
  /// run returns.
  #[must_use]
  pub fn cancel(&self) -> bool {
    self.entry.try_cancel()
  }

  pub(crate) fn entry(&self) -> ArcShared<CancellableEntry> {
    self.entry.clone()
  }
}
