//! Handle returned when scheduling jobs.

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
  #[must_use]
  pub fn is_cancelled(&self) -> bool {
    self.entry.is_cancelled()
  }

  /// Returns whether the job has completed execution.
  #[must_use]
  pub fn is_completed(&self) -> bool {
    self.entry.is_completed()
  }

  pub(crate) fn entry(&self) -> ArcShared<CancellableEntry> {
    self.entry.clone()
  }
}
