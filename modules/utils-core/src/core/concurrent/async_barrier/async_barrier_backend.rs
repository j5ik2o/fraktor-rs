use alloc::boxed::Box;

use async_trait::async_trait;

/// Trait defining the backend implementation for async barriers.
#[async_trait(?Send)]
pub trait AsyncBarrierBackend: Clone {
  /// Creates a backend that waits for the specified number of tasks.
  fn new(count: usize) -> Self;

  /// Waits at the barrier point.
  async fn wait(&self);
}
