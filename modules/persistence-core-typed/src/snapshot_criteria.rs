//! Snapshot criteria.

use fraktor_utils_core_rs::sync::ArcShared;

type SnapshotPredicate<S, E> = dyn Fn(Option<&E>, &S, u64) -> bool + Send + Sync;

/// Defines when the effector should persist a snapshot.
#[derive(Default)]
pub enum SnapshotCriteria<S, E> {
  /// Never persist snapshots automatically.
  #[default]
  Never,
  /// Persist a snapshot for every event batch.
  Always,
  /// Persist a snapshot every `number_of_events` sequence numbers.
  Every {
    /// Event interval for snapshotting.
    number_of_events: u64,
  },
  /// Persist a snapshot when the predicate returns `true`.
  Predicate(ArcShared<SnapshotPredicate<S, E>>),
}

impl<S, E> SnapshotCriteria<S, E> {
  /// Creates criteria that never snapshots automatically.
  #[must_use]
  pub const fn never() -> Self {
    Self::Never
  }

  /// Creates criteria that snapshots after every persisted batch.
  #[must_use]
  pub const fn always() -> Self {
    Self::Always
  }

  /// Creates count-based snapshot criteria.
  #[must_use]
  pub const fn every(number_of_events: u64) -> Self {
    Self::Every { number_of_events }
  }

  /// Creates predicate-based snapshot criteria.
  #[must_use]
  pub fn predicate<F>(predicate: F) -> Self
  where
    F: Fn(Option<&E>, &S, u64) -> bool + Send + Sync + 'static, {
    Self::Predicate(ArcShared::new(predicate))
  }

  /// Returns `true` when the criteria requests a snapshot.
  #[must_use]
  pub fn should_take_snapshot(&self, event: Option<&E>, state: &S, sequence_nr: u64) -> bool {
    match self {
      | Self::Never => false,
      | Self::Always => true,
      | Self::Every { number_of_events } => {
        sequence_nr > 0 && *number_of_events > 0 && sequence_nr.is_multiple_of(*number_of_events)
      },
      | Self::Predicate(predicate) => predicate(event, state, sequence_nr),
    }
  }
}

impl<S, E> Clone for SnapshotCriteria<S, E> {
  fn clone(&self) -> Self {
    match self {
      | Self::Never => Self::Never,
      | Self::Always => Self::Always,
      | Self::Every { number_of_events } => Self::Every { number_of_events: *number_of_events },
      | Self::Predicate(predicate) => Self::Predicate(predicate.clone()),
    }
  }
}
