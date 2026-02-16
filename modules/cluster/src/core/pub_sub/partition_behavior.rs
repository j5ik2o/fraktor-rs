//! Partition-time behavior for pub/sub delivery.

/// How to handle publishes while the cluster is partitioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PartitionBehavior {
  /// Queue messages to deliver after the partition heals.
  #[default]
  DelayQueue,
  /// Drop messages immediately while partitioned.
  Drop,
}
