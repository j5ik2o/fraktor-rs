//! Factory contract for [`SharedMessageQueue`](super::SharedMessageQueue).

use super::SharedMessageQueue;

/// Materializes [`SharedMessageQueue`] instances.
pub trait SharedMessageQueueFactory: Send + Sync {
  /// Creates a shared message queue.
  fn create(&self) -> SharedMessageQueue;
}
