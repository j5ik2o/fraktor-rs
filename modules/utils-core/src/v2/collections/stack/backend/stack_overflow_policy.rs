/// Policy describing how to handle stack overflow scenarios.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackOverflowPolicy {
  /// Block until space becomes available.
  Block,
  /// Grow the underlying storage capacity.
  Grow,
}
