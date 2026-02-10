use alloc::{boxed::Box, collections::VecDeque};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{DynValue, Sink, SinkDecision, SinkLogic, Source, SourceLogic, StageKind, StreamError};

#[cfg(test)]
mod tests;

/// Minimal merge hub that merges offered elements into a single queue.
pub struct MergeHub<T> {
  queue:           ArcShared<SpinSyncMutex<VecDeque<T>>>,
  receiver_active: ArcShared<SpinSyncMutex<bool>>,
  max_buffer:      usize,
}

impl<T> MergeHub<T> {
  /// Creates an empty merge hub.
  #[must_use]
  pub fn new() -> Self {
    Self {
      queue:           ArcShared::new(SpinSyncMutex::new(VecDeque::new())),
      receiver_active: ArcShared::new(SpinSyncMutex::new(false)),
      max_buffer:      16,
    }
  }

  /// Offers an element into the hub.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::WouldBlock`] when receiver side is not active or the hub buffer is
  /// full.
  pub fn offer(&self, value: T) -> Result<(), StreamError> {
    if !*self.receiver_active.lock() {
      return Err(StreamError::WouldBlock);
    }
    let mut queue = self.queue.lock();
    if queue.len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    queue.push_back(value);
    Ok(())
  }

  /// Polls the next merged element from the hub.
  #[must_use]
  pub fn poll(&self) -> Option<T> {
    *self.receiver_active.lock() = true;
    self.queue.lock().pop_front()
  }

  /// Returns the number of queued elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.queue.lock().len()
  }

  /// Returns true when the hub queue is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.queue.lock().is_empty()
  }
}

impl<T> MergeHub<T>
where
  T: Send + Sync + 'static,
{
  /// Creates a source that drains the merged queue.
  #[must_use]
  pub fn source(&self) -> Source<T, super::StreamNotUsed> {
    *self.receiver_active.lock() = true;
    Source::from_logic(StageKind::Custom, MergeHubSourceLogic {
      queue:           self.queue.clone(),
      receiver_active: self.receiver_active.clone(),
    })
  }

  /// Creates a sink that offers incoming elements into the merged queue.
  #[must_use]
  pub fn sink(&self) -> Sink<T, super::StreamNotUsed> {
    Sink::from_logic(StageKind::Custom, MergeHubSinkLogic {
      queue:           self.queue.clone(),
      receiver_active: self.receiver_active.clone(),
      max_buffer:      self.max_buffer,
    })
  }
}

impl<T> Default for MergeHub<T> {
  fn default() -> Self {
    Self::new()
  }
}

struct MergeHubSourceLogic<T> {
  queue:           ArcShared<SpinSyncMutex<VecDeque<T>>>,
  receiver_active: ArcShared<SpinSyncMutex<bool>>,
}

impl<T> SourceLogic for MergeHubSourceLogic<T>
where
  T: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    *self.receiver_active.lock() = true;
    match self.queue.lock().pop_front() {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Err(StreamError::WouldBlock),
    }
  }
}

struct MergeHubSinkLogic<T> {
  queue:           ArcShared<SpinSyncMutex<VecDeque<T>>>,
  receiver_active: ArcShared<SpinSyncMutex<bool>>,
  max_buffer:      usize,
}

impl<T> SinkLogic for MergeHubSinkLogic<T>
where
  T: Send + Sync + 'static,
{
  fn can_accept_input(&self) -> bool {
    if !*self.receiver_active.lock() {
      return false;
    }
    self.queue.lock().len() < self.max_buffer
  }

  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = super::downcast_value::<T>(input)?;
    if !*self.receiver_active.lock() {
      return Err(StreamError::WouldBlock);
    }
    let mut queue = self.queue.lock();
    if queue.len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    queue.push_back(value);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {}
}
