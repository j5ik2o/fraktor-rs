use core::task::Waker;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::base::DispatcherGeneric;

/// Adapter responsible for creating wakers and coordinating scheduler hints across runtimes.
pub trait ScheduleAdapter<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Creates a waker that reschedules the dispatcher when signalled.
  fn create_waker(&self, dispatcher: DispatcherGeneric<TB>) -> Waker;

  /// Invoked when a mailbox offer future yields `Poll::Pending`.
  fn on_pending(&self);

  /// Invoked when executor retries are exhausted and dispatcher execution is rejected.
  fn notify_rejected(&self, _attempts: usize) {}
}
