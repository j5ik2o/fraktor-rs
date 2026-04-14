//! Join-capable stop contract for running tick drivers.

use alloc::boxed::Box;

/// Join-capable stop contract for a running tick driver.
///
/// `stop(self: Box<Self>)` consumes ownership and blocks until all
/// background threads or tasks have fully stopped.
pub trait TickDriverStopper: Send + Sync + 'static {
  /// Requests shutdown and waits for all threads/tasks to complete.
  fn stop(self: Box<Self>);
}
