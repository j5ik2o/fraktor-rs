use cellactor_utils_core_rs::{
  collections::queue::{DequeBackendGeneric, QueueError},
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
};

use super::DequeHandle;

/// Handle exposing double-ended queue operations for stash consumers.
pub struct StashDequeHandleGeneric<T, TB: RuntimeToolbox + 'static = NoStdToolbox>
where
  T: Send + 'static, {
  backend: DequeBackendGeneric<T, TB>,
}

/// Type alias bound to the default toolbox.
pub type StashDequeHandle<T> = StashDequeHandleGeneric<T, NoStdToolbox>;

impl<T, TB> StashDequeHandleGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new handle from the provided backend.
  #[must_use]
  pub const fn new(backend: DequeBackendGeneric<T, TB>) -> Self {
    Self { backend }
  }

  /// Pushes an element to the front of the deque.
  pub fn push_front(&self, item: T) -> Result<(), QueueError<T>> {
    self.backend.offer_front(item).map(|_| ())
  }

  /// Pushes an element to the back of the deque.
  pub fn push_back(&self, item: T) -> Result<(), QueueError<T>> {
    self.backend.offer_back(item).map(|_| ())
  }

  /// Removes an element from the front of the deque.
  pub fn pop_front(&self) -> Result<T, QueueError<T>> {
    self.backend.poll_front()
  }

  /// Removes an element from the back of the deque.
  pub fn pop_back(&self) -> Result<T, QueueError<T>> {
    self.backend.poll_back()
  }
}

impl<T, TB> DequeHandle<T> for StashDequeHandleGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn push_front(&self, item: T) -> Result<(), QueueError<T>> {
    StashDequeHandleGeneric::push_front(self, item)
  }

  fn push_back(&self, item: T) -> Result<(), QueueError<T>> {
    StashDequeHandleGeneric::push_back(self, item)
  }

  fn pop_front(&self) -> Result<T, QueueError<T>> {
    StashDequeHandleGeneric::pop_front(self)
  }

  fn pop_back(&self) -> Result<T, QueueError<T>> {
    StashDequeHandleGeneric::pop_back(self)
  }
}
