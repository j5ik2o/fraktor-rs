//! Executor pump contract for driving scheduler work on a runtime.

use alloc::boxed::Box;
use core::time::Duration;

use super::{AutoDriverMetadata, SchedulerTickExecutor, TickDriverControl, TickDriverError, TickDriverId};

/// Drives [`SchedulerTickExecutor`] on a platform runtime.
pub trait TickExecutorPump: Send + 'static {
  /// Starts the executor pump and returns a shutdown control.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] when the runtime pump cannot be started.
  fn spawn(&mut self, executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError>;

  /// Returns auto-driver metadata to attach to the provisioned bundle when applicable.
  #[must_use]
  fn auto_metadata(&self, _driver_id: TickDriverId, _resolution: Duration) -> Option<AutoDriverMetadata> {
    None
  }
}
