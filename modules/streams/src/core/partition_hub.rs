use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{DynValue, Sink, SinkDecision, SinkLogic, Source, SourceLogic, StageKind, StreamError};

#[cfg(test)]
mod tests;

/// Minimal partition hub that routes elements into fixed partitions.
pub struct PartitionHub<T> {
  partitions: ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
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
    Self { partitions: ArcShared::new(SpinSyncMutex::new(partitions)) }
  }

  /// Offers an element to a partition selected by index.
  ///
  /// # Panics
  ///
  /// Panics when `partition` is out of range.
  pub fn offer(&self, partition: usize, value: T) {
    let mut partitions = self.partitions.lock();
    assert!(partition < partitions.len(), "partition index out of range");
    partitions[partition].push_back(value);
  }

  /// Polls the next element from the specified partition.
  #[must_use]
  pub fn poll(&self, partition: usize) -> Option<T> {
    self.partitions.lock().get_mut(partition).and_then(VecDeque::pop_front)
  }

  /// Returns partition count.
  #[must_use]
  pub fn partition_count(&self) -> usize {
    self.partitions.lock().len()
  }
}

impl<T> PartitionHub<T>
where
  T: Send + Sync + 'static,
{
  /// Creates a source that drains a specific partition.
  #[must_use]
  pub fn source_for(&self, partition: usize) -> Source<T, super::StreamNotUsed> {
    Source::from_logic(StageKind::Custom, PartitionHubSourceLogic { partitions: self.partitions.clone(), partition })
  }

  /// Creates a sink that writes to a specific partition.
  #[must_use]
  pub fn sink_for(&self, partition: usize) -> Sink<T, super::StreamNotUsed> {
    Sink::from_logic(StageKind::Custom, PartitionHubSinkLogic { partitions: self.partitions.clone(), partition })
  }
}

struct PartitionHubSourceLogic<T> {
  partitions: ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
  partition:  usize,
}

impl<T> SourceLogic for PartitionHubSourceLogic<T>
where
  T: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.partitions.lock().get_mut(self.partition).and_then(VecDeque::pop_front) {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Err(StreamError::WouldBlock),
    }
  }
}

struct PartitionHubSinkLogic<T> {
  partitions: ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
  partition:  usize,
}

impl<T> SinkLogic for PartitionHubSinkLogic<T>
where
  T: Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = super::downcast_value::<T>(input)?;
    let mut partitions = self.partitions.lock();
    assert!(self.partition < partitions.len(), "partition index out of range");
    partitions[self.partition].push_back(value);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {}
}
