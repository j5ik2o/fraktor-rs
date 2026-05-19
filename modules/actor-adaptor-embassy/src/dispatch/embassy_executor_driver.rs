//! Driver that drains tasks submitted to [`EmbassyExecutor`](super::EmbassyExecutor).

use fraktor_utils_core_rs::sync::SharedAccess;

use super::embassy_executor_shared::EmbassyExecutorShared;

/// Async driver intended to run inside an Embassy task.
pub struct EmbassyExecutorDriver<const N: usize> {
  shared: EmbassyExecutorShared<N>,
}

impl<const N: usize> EmbassyExecutorDriver<N> {
  const MAX_TASKS_PER_TURN: usize = 64;

  pub(crate) fn new(shared: EmbassyExecutorShared<N>) -> Self {
    Self { shared }
  }

  /// Drains currently queued tasks without waiting.
  pub fn drain_ready(&self) -> usize {
    self.drain_ready_with_limit(usize::MAX)
  }

  pub(crate) fn drain_ready_with_limit(&self, limit: usize) -> usize {
    let mut drained = 0;
    while drained < limit {
      let task = self.shared.with_write(|executor| executor.pop_ready());
      let Some(task) = task else {
        break;
      };
      task();
      drained += 1;
    }
    drained
  }

  /// Waits for a signal and drains a bounded batch of queued tasks.
  pub async fn run_once(&self) -> usize {
    let signal = self.shared.with_read(|executor| executor.ready_signal());
    signal.wait().await;
    let drained = self.drain_ready_with_limit(Self::MAX_TASKS_PER_TURN);
    if drained == Self::MAX_TASKS_PER_TURN {
      self.shared.with_read(|executor| executor.signal_ready());
    }
    drained
  }

  /// Runs the driver forever.
  pub async fn run(&self) -> ! {
    loop {
      self.run_once().await;
    }
  }
}
