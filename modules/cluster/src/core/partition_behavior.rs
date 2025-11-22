//! Partition-time behavior for pub/sub delivery.

/// How to handle publishes while the cluster is partitioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionBehavior {
  /// Queue messages to deliver after the partition heals.
  DelayQueue,
  /// Drop messages immediately while partitioned.
  Drop,
}

impl Default for PartitionBehavior {
  fn default() -> Self {
    Self::DelayQueue
  }
}
