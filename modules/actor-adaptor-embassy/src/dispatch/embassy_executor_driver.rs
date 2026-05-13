//! Driver that drains tasks submitted to [`EmbassyExecutor`](super::EmbassyExecutor).

use super::embassy_executor_shared::EmbassyExecutorShared;

/// Async driver intended to run inside an Embassy task.
pub struct EmbassyExecutorDriver<const N: usize> {
  shared: EmbassyExecutorShared<N>,
}

impl<const N: usize> EmbassyExecutorDriver<N> {
  pub(crate) const fn new(shared: EmbassyExecutorShared<N>) -> Self {
    Self { shared }
  }

  /// Drains currently queued tasks without waiting.
  pub fn drain_ready(&self) -> usize {
    self.shared.drain_ready()
  }

  /// Waits for a signal and drains all currently queued tasks.
  pub async fn run_once(&self) -> usize {
    self.shared.wait_ready().await;
    self.shared.drain_ready()
  }

  /// Runs the driver forever.
  pub async fn run(&self) -> ! {
    loop {
      self.run_once().await;
    }
  }
}
