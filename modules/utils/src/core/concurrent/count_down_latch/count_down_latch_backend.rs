use alloc::boxed::Box;

use async_trait::async_trait;

/// Trait defining the backend implementation for CountDownLatch.
#[async_trait(?Send)]
pub trait CountDownLatchBackend: Clone {
  /// Initializes the backend with the specified count value.
  fn new(count: usize) -> Self;

  /// Decrements the count by 1.
  async fn count_down(&self);

  /// Waits until the count reaches 0.
  async fn wait(&self);
}
