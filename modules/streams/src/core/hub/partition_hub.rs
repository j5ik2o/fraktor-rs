use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{DynValue, Sink, SinkDecision, SinkLogic, Source, SourceLogic, StageKind, StreamError};

#[cfg(test)]
mod tests;

/// Minimal partition hub that routes elements into fixed partitions.
pub struct PartitionHub<T> {
  partitions:       ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
  active_consumers: ArcShared<SpinSyncMutex<Vec<bool>>>,
  max_buffer:       usize,
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
    let mut active_consumers = Vec::with_capacity(partition_count);
    for _ in 0..partition_count {
      partitions.push(VecDeque::new());
      active_consumers.push(false);
    }
    Self {
      partitions:       ArcShared::new(SpinSyncMutex::new(partitions)),
      active_consumers: ArcShared::new(SpinSyncMutex::new(active_consumers)),
      max_buffer:       16,
    }
  }

  /// Offers an element to a partition selected by index.
  ///
  /// # Panics
  ///
  /// Panics when `partition` is out of range.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::WouldBlock`] when the partition has no active consumer or buffer is
  /// full.
  pub fn offer(&self, partition: usize, value: T) -> Result<(), StreamError> {
    {
      let active_consumers = self.active_consumers.lock();
      assert!(partition < active_consumers.len(), "partition index out of range");
      if !active_consumers[partition] {
        return Err(StreamError::WouldBlock);
      }
    }
    let mut partitions = self.partitions.lock();
    assert!(partition < partitions.len(), "partition index out of range");
    if partitions[partition].len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    partitions[partition].push_back(value);
    Ok(())
  }

  /// Routes an element by using the provided partitioner.
  ///
  /// The partitioner receives the number of active consumers and must return a valid partition
  /// index.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::WouldBlock`] when there is no active consumer.
  /// Returns [`StreamError::InvalidRoute`] when the route is negative, out of range, or points to
  /// an inactive consumer.
  pub fn route_with<F>(&self, value: T, partitioner: F) -> Result<(), StreamError>
  where
    F: FnOnce(usize) -> isize, {
    let active_consumers = self.active_consumers.lock();
    let partition_count = active_consumers.len();
    let active_consumer_count = active_consumers.iter().filter(|&&is_active| is_active).count();
    if active_consumer_count == 0 {
      return Err(StreamError::WouldBlock);
    }

    let route = partitioner(active_consumer_count);
    if route < 0 {
      return Err(StreamError::InvalidRoute { route, partition_count });
    }
    let partition = route as usize;
    if partition >= partition_count || !active_consumers[partition] {
      return Err(StreamError::InvalidRoute { route, partition_count });
    }
    drop(active_consumers);

    let mut partitions = self.partitions.lock();
    if partitions[partition].len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    partitions[partition].push_back(value);
    Ok(())
  }

  /// Polls the next element from the specified partition.
  #[must_use]
  pub fn poll(&self, partition: usize) -> Option<T> {
    {
      let mut active_consumers = self.active_consumers.lock();
      if partition < active_consumers.len() {
        active_consumers[partition] = true;
      }
    }
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
  ///
  /// # Panics
  ///
  /// Panics when `partition` is out of range.
  #[must_use]
  pub fn source_for(&self, partition: usize) -> Source<T, super::StreamNotUsed> {
    {
      let mut active_consumers = self.active_consumers.lock();
      assert!(partition < active_consumers.len(), "partition index out of range");
      active_consumers[partition] = true;
    }
    Source::from_logic(StageKind::Custom, PartitionHubSourceLogic {
      partitions: self.partitions.clone(),
      active_consumers: self.active_consumers.clone(),
      partition,
    })
  }

  /// Creates a sink that writes to a specific partition.
  #[must_use]
  pub fn sink_for(&self, partition: usize) -> Sink<T, super::StreamNotUsed> {
    Sink::from_logic(StageKind::Custom, PartitionHubSinkLogic {
      partitions: self.partitions.clone(),
      active_consumers: self.active_consumers.clone(),
      partition,
      max_buffer: self.max_buffer,
    })
  }
}

struct PartitionHubSourceLogic<T> {
  partitions:       ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
  active_consumers: ArcShared<SpinSyncMutex<Vec<bool>>>,
  partition:        usize,
}

impl<T> SourceLogic for PartitionHubSourceLogic<T>
where
  T: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    {
      let mut active_consumers = self.active_consumers.lock();
      if self.partition < active_consumers.len() {
        active_consumers[self.partition] = true;
      }
    }
    match self.partitions.lock().get_mut(self.partition).and_then(VecDeque::pop_front) {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Err(StreamError::WouldBlock),
    }
  }
}

struct PartitionHubSinkLogic<T> {
  partitions:       ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
  active_consumers: ArcShared<SpinSyncMutex<Vec<bool>>>,
  partition:        usize,
  max_buffer:       usize,
}

impl<T> SinkLogic for PartitionHubSinkLogic<T>
where
  T: Send + Sync + 'static,
{
  fn can_accept_input(&self) -> bool {
    let active_consumers = self.active_consumers.lock();
    if self.partition >= active_consumers.len() || !active_consumers[self.partition] {
      return false;
    }
    drop(active_consumers);
    let partitions = self.partitions.lock();
    self.partition < partitions.len() && partitions[self.partition].len() < self.max_buffer
  }

  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = super::downcast_value::<T>(input)?;
    let active_consumers = self.active_consumers.lock();
    if self.partition >= active_consumers.len() || !active_consumers[self.partition] {
      return Err(StreamError::WouldBlock);
    }
    drop(active_consumers);
    let mut partitions = self.partitions.lock();
    assert!(self.partition < partitions.len(), "partition index out of range");
    if partitions[self.partition].len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    partitions[self.partition].push_back(value);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {}
}
