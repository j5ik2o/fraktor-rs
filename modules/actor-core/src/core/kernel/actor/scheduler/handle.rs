//! Handle returned when scheduling jobs.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::cancellable::CancellableEntry;

/// Identifier for scheduled jobs.
///
/// This is also exposed as the Pekko-compatible
/// [`Cancellable`](crate::core::kernel::actor::scheduler::Cancellable) alias.
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
  /// Returns `true` only when the entry is in the `Scheduled` state and the
  /// cancellation transition succeeds. Returns `false` in all other cases:
  /// `Pending` (not yet scheduled), `Executing` (already running), or when
  /// already in a terminal state (`Cancelled` or `Completed`).
  ///
  /// Corresponds to Pekko's `Cancellable.cancel`.
  ///
  /// Note: this marks the entry as cancelled but does not remove it from the
  /// scheduler's job queue. The scheduler will skip cancelled entries on the
  /// next tick.
  #[must_use]
  pub fn cancel(&self) -> bool {
    self.entry.try_cancel()
  }

  pub(crate) fn entry(&self) -> ArcShared<CancellableEntry> {
    self.entry.clone()
  }
}
