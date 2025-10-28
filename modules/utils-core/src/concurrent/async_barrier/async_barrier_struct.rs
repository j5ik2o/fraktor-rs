use super::async_barrier_backend::AsyncBarrierBackend;

/// Structure providing synchronization barrier among async tasks.
#[derive(Clone, Debug)]
pub struct AsyncBarrier<B>
where
  B: AsyncBarrierBackend, {
  backend: B,
}

impl<B> AsyncBarrier<B>
where
  B: AsyncBarrierBackend,
{
  /// Creates a new barrier that waits for the specified number of tasks.
  #[must_use]
  pub fn new(count: usize) -> Self {
    Self { backend: B::new(count) }
  }

  /// Waits at the barrier point.
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// Gets a reference to the backend implementation.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }
}
