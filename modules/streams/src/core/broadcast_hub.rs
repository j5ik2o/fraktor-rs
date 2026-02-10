use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{DynValue, Sink, SinkDecision, SinkLogic, Source, SourceLogic, StageKind, StreamError};

#[cfg(test)]
mod tests;

/// Minimal broadcast hub that fans out each element to every subscriber.
pub struct BroadcastHub<T> {
  subscribers: ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
  max_buffer:  usize,
}

impl<T> BroadcastHub<T>
where
  T: Clone,
{
  /// Creates an empty broadcast hub.
  #[must_use]
  pub fn new() -> Self {
    Self { subscribers: ArcShared::new(SpinSyncMutex::new(Vec::new())), max_buffer: 16 }
  }

  /// Adds a subscriber and returns its identifier.
  #[must_use]
  pub fn subscribe(&self) -> usize {
    let mut subscribers = self.subscribers.lock();
    subscribers.push(VecDeque::new());
    subscribers.len().saturating_sub(1)
  }

  /// Publishes an element to all subscribers.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::WouldBlock`] when no subscriber is active or any subscriber queue is
  /// full.
  pub fn publish(&self, value: T) -> Result<(), StreamError> {
    let mut subscribers = self.subscribers.lock();
    if subscribers.is_empty() {
      return Err(StreamError::WouldBlock);
    }
    if subscribers.iter().any(|queue| queue.len() >= self.max_buffer) {
      return Err(StreamError::WouldBlock);
    }
    for queue in &mut *subscribers {
      queue.push_back(value.clone());
    }
    Ok(())
  }

  /// Polls the next element for the specified subscriber.
  #[must_use]
  pub fn poll(&self, subscriber_id: usize) -> Option<T> {
    self.subscribers.lock().get_mut(subscriber_id).and_then(VecDeque::pop_front)
  }
}

impl<T> BroadcastHub<T>
where
  T: Clone + Send + Sync + 'static,
{
  /// Creates a source for a specific subscriber queue.
  #[must_use]
  pub fn source_for(&self, subscriber_id: usize) -> Source<T, super::StreamNotUsed> {
    Source::from_logic(StageKind::Custom, BroadcastHubSourceLogic {
      subscribers: self.subscribers.clone(),
      subscriber_id,
    })
  }

  /// Creates a sink that publishes every element to all subscribers.
  #[must_use]
  pub fn sink(&self) -> Sink<T, super::StreamNotUsed> {
    Sink::from_logic(StageKind::Custom, BroadcastHubSinkLogic {
      subscribers: self.subscribers.clone(),
      max_buffer:  self.max_buffer,
    })
  }
}

impl<T> Default for BroadcastHub<T>
where
  T: Clone,
{
  fn default() -> Self {
    Self::new()
  }
}

struct BroadcastHubSourceLogic<T> {
  subscribers:   ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
  subscriber_id: usize,
}

impl<T> SourceLogic for BroadcastHubSourceLogic<T>
where
  T: Clone + Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.subscribers.lock().get_mut(self.subscriber_id).and_then(VecDeque::pop_front) {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Err(StreamError::WouldBlock),
    }
  }
}

struct BroadcastHubSinkLogic<T> {
  subscribers: ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
  max_buffer:  usize,
}

impl<T> SinkLogic for BroadcastHubSinkLogic<T>
where
  T: Clone + Send + Sync + 'static,
{
  fn can_accept_input(&self) -> bool {
    let subscribers = self.subscribers.lock();
    !subscribers.is_empty() && subscribers.iter().all(|queue| queue.len() < self.max_buffer)
  }

  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = super::downcast_value::<T>(input)?;
    let mut subscribers = self.subscribers.lock();
    if subscribers.is_empty() {
      return Err(StreamError::WouldBlock);
    }
    if subscribers.iter().any(|queue| queue.len() >= self.max_buffer) {
      return Err(StreamError::WouldBlock);
    }
    for queue in &mut *subscribers {
      queue.push_back(value.clone());
    }
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {}
}
