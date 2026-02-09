use alloc::{collections::VecDeque, vec::Vec};

#[cfg(test)]
mod tests;

/// Minimal partition hub that routes elements into fixed partitions.
pub struct PartitionHub<T> {
  partitions: Vec<VecDeque<T>>,
}

impl<T> PartitionHub<T> {
  /// Creates a partition hub with the specified partition count.
  ///
  /// # Panics
  ///
  /// Panics when `partition_count` is zero.
  #[must_use]
  pub fn new(partition_count: usize) -> Self {
    assert!(partition_count > 0, "partition_count must be greater than zero");
    let mut partitions = Vec::with_capacity(partition_count);
    for _ in 0..partition_count {
      partitions.push(VecDeque::new());
    }
    Self { partitions }
  }

  /// Offers an element to a partition selected by index.
  ///
  /// # Panics
  ///
  /// Panics when `partition` is out of range.
  pub fn offer(&mut self, partition: usize, value: T) {
    assert!(partition < self.partitions.len(), "partition index out of range");
    self.partitions[partition].push_back(value);
  }

  /// Polls the next element from the specified partition.
  #[must_use]
  pub fn poll(&mut self, partition: usize) -> Option<T> {
    self.partitions.get_mut(partition).and_then(VecDeque::pop_front)
  }

  /// Returns partition count.
  #[must_use]
  pub const fn partition_count(&self) -> usize {
    self.partitions.len()
  }
}
