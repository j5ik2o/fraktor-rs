use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{DynValue, Sink, SinkDecision, SinkLogic, Source, SourceLogic, StageKind, StreamError};

#[cfg(test)]
mod tests;

/// Combined state to avoid TOCTOU races between active_consumers/partitions checks.
struct PartitionHubState<T> {
  partitions:       Vec<VecDeque<T>>,
  active_consumers: Vec<bool>,
}

/// Minimal partition hub that routes elements into fixed partitions.
pub struct PartitionHub<T> {
  state:      ArcShared<SpinSyncMutex<PartitionHubState<T>>>,
  max_buffer: usize,
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
      state:      ArcShared::new(SpinSyncMutex::new(PartitionHubState { partitions, active_consumers })),
      max_buffer: 16,
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
    let mut guard = self.state.lock();
    assert!(partition < guard.active_consumers.len(), "partition index out of range");
    if !guard.active_consumers[partition] {
      return Err(StreamError::WouldBlock);
    }
    assert!(partition < guard.partitions.len(), "partition index out of range");
    if guard.partitions[partition].len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    guard.partitions[partition].push_back(value);
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
    let mut guard = self.state.lock();
    let partition_count = guard.active_consumers.len();
    let active_consumer_count = guard.active_consumers.iter().filter(|&&is_active| is_active).count();
    if active_consumer_count == 0 {
      return Err(StreamError::WouldBlock);
    }

    let route = partitioner(active_consumer_count);
    if route < 0 || route as usize >= active_consumer_count {
      return Err(StreamError::InvalidRoute { route, partition_count });
    }

    // N番目のアクティブコンシューマを実パーティションインデックスに変換
    let nth_active = route as usize;
    let partition = guard
      .active_consumers
      .iter()
      .enumerate()
      .filter(|&(_, is_active)| *is_active)
      .nth(nth_active)
      .map(|(index, _)| index)
      .ok_or(StreamError::InvalidRoute { route, partition_count })?;

    if guard.partitions[partition].len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    guard.partitions[partition].push_back(value);
    Ok(())
  }

  /// Polls the next element from the specified partition.
  #[must_use]
  pub fn poll(&self, partition: usize) -> Option<T> {
    let mut guard = self.state.lock();
    if partition < guard.active_consumers.len() {
      guard.active_consumers[partition] = true;
    }
    guard.partitions.get_mut(partition).and_then(VecDeque::pop_front)
  }

  /// Returns partition count.
  #[must_use]
  pub fn partition_count(&self) -> usize {
    self.state.lock().partitions.len()
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
      let mut guard = self.state.lock();
      assert!(partition < guard.active_consumers.len(), "partition index out of range");
      guard.active_consumers[partition] = true;
    }
    Source::from_logic(StageKind::Custom, PartitionHubSourceLogic { state: self.state.clone(), partition })
  }

  /// Creates a sink that writes to a specific partition.
  #[must_use]
  pub fn sink_for(&self, partition: usize) -> Sink<T, super::StreamNotUsed> {
    Sink::from_logic(StageKind::Custom, PartitionHubSinkLogic {
      state: self.state.clone(),
      partition,
      max_buffer: self.max_buffer,
    })
  }
}

struct PartitionHubSourceLogic<T> {
  state:     ArcShared<SpinSyncMutex<PartitionHubState<T>>>,
  partition: usize,
}

impl<T> SourceLogic for PartitionHubSourceLogic<T>
where
  T: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    let mut guard = self.state.lock();
    if self.partition < guard.active_consumers.len() {
      guard.active_consumers[self.partition] = true;
    }
    match guard.partitions.get_mut(self.partition).and_then(VecDeque::pop_front) {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Err(StreamError::WouldBlock),
    }
  }
}

struct PartitionHubSinkLogic<T> {
  state:      ArcShared<SpinSyncMutex<PartitionHubState<T>>>,
  partition:  usize,
  max_buffer: usize,
}

impl<T> SinkLogic for PartitionHubSinkLogic<T>
where
  T: Send + Sync + 'static,
{
  fn can_accept_input(&self) -> bool {
    let guard = self.state.lock();
    if self.partition >= guard.active_consumers.len() || !guard.active_consumers[self.partition] {
      return false;
    }
    self.partition < guard.partitions.len() && guard.partitions[self.partition].len() < self.max_buffer
  }

  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = super::downcast_value::<T>(input)?;
    let mut guard = self.state.lock();
    if self.partition >= guard.active_consumers.len() || !guard.active_consumers[self.partition] {
      return Err(StreamError::WouldBlock);
    }
    assert!(self.partition < guard.partitions.len(), "partition index out of range");
    if guard.partitions[self.partition].len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    guard.partitions[self.partition].push_back(value);
    drop(guard);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {}
}
