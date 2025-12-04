use core::{any::Any, task::Waker};

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::base::DispatcherGeneric;

/// Adapter responsible for creating wakers and coordinating scheduler hints across runtimes.
pub trait ScheduleAdapter<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Creates a waker that reschedules the dispatcher when signalled.
  fn create_waker(&mut self, dispatcher: DispatcherGeneric<TB>) -> Waker;

  /// Invoked when a mailbox offer future yields `Poll::Pending`.
  fn on_pending(&mut self);

  /// Invoked when executor retries are exhausted and dispatcher execution is rejected.
  fn notify_rejected(&mut self, _attempts: usize) {}

  /// Downcasts to the concrete type for testing or diagnostics.
  fn as_any_mut(&mut self) -> &mut dyn Any
  where
    Self: 'static;
}
