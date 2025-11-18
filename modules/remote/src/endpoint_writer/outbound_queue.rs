//! Priority queue separating system and user messages.

use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  collections::queue::{OverflowPolicy, QueueError, SyncFifoQueue, backend::VecDequeBackend},
  runtime_toolbox::RuntimeToolbox,
};

/// Differentiates system vs user envelopes.
pub enum EnvelopePriority {
  /// System messages have higher priority.
  System,
  /// User messages are processed after system queues drain.
  User,
}

/// Queues outbound envelopes while respecting system priority and backpressure signals.
pub struct OutboundQueue<TB: RuntimeToolbox + 'static, T> {
  system:      SyncFifoQueue<T, VecDequeBackend<T>>,
  user:        SyncFifoQueue<T, VecDequeBackend<T>>,
  user_paused: bool,
  _marker:     PhantomData<TB>,
}

const DEFAULT_QUEUE_CAPACITY: usize = 256;

impl<TB: RuntimeToolbox + 'static, T> OutboundQueue<TB, T> {
  /// Creates an empty queue.
  #[must_use]
  pub fn new() -> Self {
    Self {
      system:      SyncFifoQueue::new(VecDequeBackend::with_capacity(DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow)),
      user:        SyncFifoQueue::new(VecDequeBackend::with_capacity(DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow)),
      user_paused: false,
      _marker:     PhantomData,
    }
  }

  /// Enqueues the payload using the provided priority classifier.
  pub fn push<F>(&mut self, item: T, classify: F) -> Result<(), QueueError<T>>
  where
    F: FnOnce(&T) -> EnvelopePriority, {
    match classify(&item) {
      | EnvelopePriority::System => Self::offer(&mut self.system, item),
      | EnvelopePriority::User => Self::offer(&mut self.user, item),
    }
  }

  /// Pops the next element, draining system queue before user queue.
  #[must_use]
  pub fn pop(&mut self) -> Result<Option<T>, QueueError<T>> {
    if let Some(item) = Self::poll(&mut self.system)? {
      return Ok(Some(item));
    }
    if self.user_paused {
      return Ok(None);
    }
    Self::poll(&mut self.user)
  }

  /// Pauses draining of user messages while honoring system priority.
  pub fn pause_user(&mut self) {
    self.user_paused = true;
  }

  /// Resumes draining of user messages.
  pub fn resume_user(&mut self) {
    self.user_paused = false;
  }

  fn offer(queue: &mut SyncFifoQueue<T, VecDequeBackend<T>>, item: T) -> Result<(), QueueError<T>> {
    queue.offer(item).map(|_| ())
  }

  fn poll(queue: &mut SyncFifoQueue<T, VecDequeBackend<T>>) -> Result<Option<T>, QueueError<T>> {
    match queue.poll() {
      | Ok(item) => Ok(Some(item)),
      | Err(QueueError::Empty) => Ok(None),
      | Err(err) => Err(err),
    }
  }
}
