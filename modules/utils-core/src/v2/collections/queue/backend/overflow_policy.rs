/// Policy describing how to handle capacity overflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowPolicy {
  /// Drop the newest items when capacity is exhausted.
  DropNewest,
  /// Drop the oldest items when capacity is exhausted.
  DropOldest,
  /// Block the caller until space becomes available.
  Block,
  /// Grow the underlying storage capacity.
  Grow,
}

impl Default for OverflowPolicy {
  fn default() -> Self {
    OverflowPolicy::DropOldest
  }
}
