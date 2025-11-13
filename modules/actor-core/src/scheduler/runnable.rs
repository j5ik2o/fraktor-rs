//! Runnable trait executed by scheduler-delivered closures.

/// Trait implemented by runnable tasks scheduled through the scheduler APIs.
pub trait SchedulerRunnable: Send + Sync + 'static {
  /// Executes the runnable.
  fn run(&self);
}

impl<F> SchedulerRunnable for F
where
  F: Fn() + Send + Sync + 'static,
{
  fn run(&self) {
    (self)();
  }
}
