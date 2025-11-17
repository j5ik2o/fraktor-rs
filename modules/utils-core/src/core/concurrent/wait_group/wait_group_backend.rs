use alloc::boxed::Box;

use async_trait::async_trait;

/// Backend trait for wait-group implementations.
#[async_trait(?Send)]
pub trait WaitGroupBackend: Clone {
  /// Creates a new backend instance.
  fn new() -> Self;

  /// Creates a new backend instance with the specified count.
  fn with_count(count: usize) -> Self;

  /// Adds the specified number to the counter.
  fn add(&self, n: usize);

  /// Decrements the counter by 1.
  fn done(&self);

  /// Waits until the counter reaches 0.
  async fn wait(&self);
}
