use super::count_down_latch_backend::CountDownLatchBackend;

/// Count-down latch synchronization primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountDownLatch<B>
where
  B: CountDownLatchBackend, {
  backend: B,
}

impl<B> CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  /// Creates a new `CountDownLatch` with the specified count value.
  #[must_use]
  pub fn new(count: usize) -> Self {
    Self { backend: B::new(count) }
  }

  /// Decrements the count by 1.
  pub async fn count_down(&self) {
    self.backend.count_down().await;
  }

  /// Causes the current task to wait until the count reaches 0.
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// Gets a reference to the internal backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }
}

impl<B> Default for CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  fn default() -> Self {
    Self::new(0)
  }
}
