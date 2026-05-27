//! Retention criteria.

/// Defines how many snapshots should remain after snapshot persistence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetentionCriteria {
  snapshot_every:            Option<u64>,
  keep_snapshots:            Option<u64>,
  delete_events_on_snapshot: bool,
}

impl RetentionCriteria {
  /// Creates snapshot-count based retention criteria.
  #[must_use]
  pub const fn snapshot_every(number_of_events: u64, keep_snapshots: u64) -> Self {
    Self {
      snapshot_every:            Some(number_of_events),
      keep_snapshots:            Some(keep_snapshots),
      delete_events_on_snapshot: false,
    }
  }

  /// Returns the snapshot interval used for retention.
  #[must_use]
  pub const fn snapshot_every_interval(&self) -> Option<u64> {
    self.snapshot_every
  }

  /// Returns the number of snapshots to keep.
  #[must_use]
  pub const fn keep_snapshots(&self) -> Option<u64> {
    self.keep_snapshots
  }

  /// Returns whether events are deleted after retention snapshots.
  #[must_use]
  pub const fn delete_events_on_snapshot(&self) -> bool {
    self.delete_events_on_snapshot
  }

  /// Returns criteria that deletes old events when retention snapshots are saved.
  #[must_use]
  pub const fn with_delete_events_on_snapshot(mut self) -> Self {
    self.delete_events_on_snapshot = true;
    self
  }

  pub(crate) fn delete_to_sequence_nr(&self, sequence_nr: u64) -> Option<u64> {
    let snapshot_every = self.snapshot_every_interval()?;
    let keep_snapshots = self.keep_snapshots()?;
    if snapshot_every == 0 || keep_snapshots == 0 {
      return None;
    }
    let latest_snapshot_sequence_nr = sequence_nr.checked_sub(sequence_nr % snapshot_every)?;
    if latest_snapshot_sequence_nr < snapshot_every {
      return None;
    }
    let kept_snapshot_span = snapshot_every.checked_mul(keep_snapshots.saturating_sub(1))?;
    let oldest_kept_snapshot = latest_snapshot_sequence_nr.checked_sub(kept_snapshot_span)?;
    if oldest_kept_snapshot == 0 {
      return None;
    }
    let max_sequence_nr_to_delete = oldest_kept_snapshot.checked_sub(snapshot_every)?;
    (max_sequence_nr_to_delete > 0).then_some(max_sequence_nr_to_delete)
  }
}
