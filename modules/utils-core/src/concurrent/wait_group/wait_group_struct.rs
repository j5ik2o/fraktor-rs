use super::wait_group_backend::WaitGroupBackend;

/// Synchronization primitive for waiting on multiple concurrent tasks.
#[derive(Clone, Debug)]
pub struct WaitGroup<B>
where
  B: WaitGroupBackend, {
  backend: B,
}

impl<B> WaitGroup<B>
where
  B: WaitGroupBackend,
{
  /// Creates a new wait group with counter initialised to 0.
  #[must_use]
  pub fn new() -> Self {
    Self { backend: B::new() }
  }

  /// Creates a new wait group with the specified count.
  #[must_use]
  pub fn with_count(count: usize) -> Self {
    Self { backend: B::with_count(count) }
  }

  /// Adds the specified number to the counter.
  pub fn add(&self, n: usize) {
    self.backend.add(n);
  }

  /// Decrements the counter by 1.
  pub fn done(&self) {
    self.backend.done();
  }

  /// Asynchronously waits until the counter reaches 0.
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// Gets a reference to the backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }
}

impl<B> Default for WaitGroup<B>
where
  B: WaitGroupBackend,
{
  fn default() -> Self {
    Self::new()
  }
}
