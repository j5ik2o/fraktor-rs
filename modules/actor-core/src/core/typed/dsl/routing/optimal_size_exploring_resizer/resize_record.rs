//! Bookkeeping snapshot maintained between resize decisions.

use super::under_utilization_streak::UnderUtilizationStreak;

/// Snapshot of router statistics retained between `report_message_count` calls.
///
/// Corresponds to Pekko's `OptimalSizeExploringResizer.ResizeRecord`.
///
/// # Sentinel replacement
///
/// Pekko uses `checkTime = 0L` as a sentinel meaning "no baseline has been
/// recorded yet". Because our [`Clock`] abstraction makes `Instant` an
/// associated type, an `Instant` value of "zero" is not necessarily invalid.
/// Instead we carry an explicit [`has_recorded`](Self::has_recorded) flag and
/// a real [`check_time`](Self::check_time) initialized at construction.
pub(crate) struct ResizeRecord<I> {
  /// Active under-utilization streak, if any.
  pub(crate) under_utilization_streak: Option<UnderUtilizationStreak<I>>,
  /// Cumulative message counter observed at the previous sample.
  pub(crate) message_count:            u64,
  /// Total pending mailbox size observed at the previous sample.
  pub(crate) total_queue_length:       u64,
  /// `true` once at least one sample has been recorded.
  ///
  /// Replaces Pekko's `checkTime > 0` gate that prevents perf_log updates
  /// from using an uninitialized baseline.
  pub(crate) has_recorded:             bool,
  /// Instant at which the previous sample was recorded.
  pub(crate) check_time:               I,
}
