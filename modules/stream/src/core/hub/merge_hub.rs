use alloc::{boxed::Box, collections::VecDeque};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{DrainingControl, DynValue, Sink, SinkDecision, SinkLogic, Source, SourceLogic, StageKind, StreamError};

#[cfg(test)]
mod tests;

/// Internal state protected by a single mutex to avoid TOCTOU races.
struct MergeHubState<T> {
  queue:           VecDeque<T>,
  receiver_active: bool,
  draining:        bool,
}

/// Minimal merge hub that merges offered elements into a single queue.
pub struct MergeHub<T> {
  state:      ArcShared<SpinSyncMutex<MergeHubState<T>>>,
  max_buffer: usize,
}

impl<T> MergeHub<T> {
  /// Creates an empty merge hub.
  #[must_use]
  pub fn new() -> Self {
    Self {
      state:      ArcShared::new(SpinSyncMutex::new(MergeHubState {
        queue:           VecDeque::new(),
        receiver_active: false,
        draining:        false,
      })),
      max_buffer: 16,
    }
  }

  /// Offers an element into the hub.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::WouldBlock`] when receiver side is not active or the hub buffer is
  /// full.
  pub fn offer(&self, value: T) -> Result<(), StreamError> {
    let mut guard = self.state.lock();
    if !guard.receiver_active {
      return Err(StreamError::WouldBlock);
    }
    if guard.draining {
      return Err(StreamError::WouldBlock);
    }
    if guard.queue.len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    guard.queue.push_back(value);
    Ok(())
  }

  /// Polls the next merged element from the hub.
  #[must_use]
  pub fn poll(&self) -> Option<T> {
    let mut guard = self.state.lock();
    guard.receiver_active = true;
    guard.queue.pop_front()
  }

  /// Returns the number of queued elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.state.lock().queue.len()
  }

  /// Returns true when the hub queue is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.state.lock().queue.is_empty()
  }
}

impl<T> MergeHub<T>
where
  T: Send + Sync + 'static,
{
  /// Returns a control handle that can start draining mode.
  #[must_use]
  pub fn draining_control(&self) -> DrainingControl {
    let drain_state = self.state.clone();
    let query_state = self.state.clone();
    DrainingControl::new_with_callback(
      move || {
        drain_state.lock().draining = true;
      },
      move || query_state.lock().draining,
    )
  }

  /// Creates a source that drains the merged queue.
  #[must_use]
  pub fn source(&self) -> Source<T, super::StreamNotUsed> {
    self.state.lock().receiver_active = true;
    Source::from_logic(StageKind::Custom, MergeHubSourceLogic { state: self.state.clone() })
  }

  /// Creates a sink that offers incoming elements into the merged queue.
  #[must_use]
  pub fn sink(&self) -> Sink<T, super::StreamNotUsed> {
    Sink::from_logic(StageKind::Custom, MergeHubSinkLogic {
      state:      self.state.clone(),
      max_buffer: self.max_buffer,
    })
  }
}

impl<T> Default for MergeHub<T> {
  fn default() -> Self {
    Self::new()
  }
}

struct MergeHubSourceLogic<T> {
  state: ArcShared<SpinSyncMutex<MergeHubState<T>>>,
}

impl<T> SourceLogic for MergeHubSourceLogic<T>
where
  T: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    let mut guard = self.state.lock();
    guard.receiver_active = true;
    match guard.queue.pop_front() {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => {
        if guard.draining {
          return Ok(None);
        }
        Err(StreamError::WouldBlock)
      },
    }
  }
}

struct MergeHubSinkLogic<T> {
  state:      ArcShared<SpinSyncMutex<MergeHubState<T>>>,
  max_buffer: usize,
}

impl<T> SinkLogic for MergeHubSinkLogic<T>
where
  T: Send + Sync + 'static,
{
  fn can_accept_input(&self) -> bool {
    let guard = self.state.lock();
    guard.receiver_active && !guard.draining && guard.queue.len() < self.max_buffer
  }

  fn on_start(&mut self, demand: &mut super::DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut super::DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = super::downcast_value::<T>(input)?;
    let mut guard = self.state.lock();
    if !guard.receiver_active || guard.draining {
      return Err(StreamError::WouldBlock);
    }
    if guard.queue.len() >= self.max_buffer {
      return Err(StreamError::WouldBlock);
    }
    guard.queue.push_back(value);
    drop(guard);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {}
}
