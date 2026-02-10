use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{DynValue, Sink, SinkDecision, SinkLogic, Source, SourceLogic, StageKind, StreamError};

#[cfg(test)]
mod tests;

/// Minimal broadcast hub that fans out each element to every subscriber.
pub struct BroadcastHub<T> {
  subscribers: ArcShared<SpinSyncMutex<Vec<VecDeque<T>>>>,
}

impl<T> BroadcastHub<T>
where
  T: Clone,
{
  /// Creates an empty broadcast hub.
  #[must_use]
  pub fn new() -> Self {
    Self { subscribers: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  /// Adds a subscriber and returns its identifier.
  #[must_use]
  pub fn subscribe(&self) -> usize {
    let mut subscribers = self.subscribers.lock();
    subscribers.push(VecDeque::new());
    subscribers.len().saturating_sub(1)
  }

  /// Publishes an element to all subscribers.
  pub fn publish(&self, value: T) {
    for queue in &mut *self.subscribers.lock() {
      queue.push_back(value.clone());
    }
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
    Sink::from_logic(StageKind::Custom, BroadcastHubSinkLogic { subscribers: self.subscribers.clone() })
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
}

impl<T> SinkLogic for BroadcastHubSinkLogic<T>
where
  T: Clone + Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = super::downcast_value::<T>(input)?;
    for queue in &mut *self.subscribers.lock() {
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
