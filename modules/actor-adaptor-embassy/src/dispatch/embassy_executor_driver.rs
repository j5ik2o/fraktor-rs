//! Driver that drains tasks submitted to [`EmbassyExecutor`](super::EmbassyExecutor).

use fraktor_utils_core_rs::sync::SharedAccess;

use super::embassy_executor_shared::EmbassyExecutorShared;

/// Async driver intended to run inside an Embassy task.
pub struct EmbassyExecutorDriver<const N: usize> {
  shared: EmbassyExecutorShared<N>,
}

impl<const N: usize> EmbassyExecutorDriver<N> {
  pub(crate) fn new(shared: EmbassyExecutorShared<N>) -> Self {
    Self { shared }
  }

  /// Drains currently queued tasks without waiting.
  pub fn drain_ready(&self) -> usize {
    let mut drained = 0;
    while let Some(task) = self.shared.with_write(|executor| executor.pop_ready()) {
      task();
      drained += 1;
    }
    drained
  }

  /// Waits for a signal and drains all currently queued tasks.
  pub async fn run_once(&self) -> usize {
    let signal = self.shared.with_read(|executor| executor.ready_signal());
    signal.wait().await;
    self.drain_ready()
  }

  /// Runs the driver forever.
  pub async fn run(&self) -> ! {
    loop {
      self.run_once().await;
    }
  }
}
