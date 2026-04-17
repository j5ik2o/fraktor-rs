//! Internal record of an ongoing under-utilization period.

/// Tracks a contiguous period during which the pool was not fully utilized.
///
/// Corresponds to Pekko's `OptimalSizeExploringResizer.UnderUtilizationStreak`.
/// The streak is reset (set back to `None`) whenever the pool becomes fully
/// utilized, and extended otherwise.
#[derive(Debug, Clone, Copy)]
pub(crate) struct UnderUtilizationStreak<I> {
  /// Timestamp when the streak started.
  pub(crate) start:               I,
  /// Highest number of busy routees observed during the streak.
  pub(crate) highest_utilization: usize,
}
