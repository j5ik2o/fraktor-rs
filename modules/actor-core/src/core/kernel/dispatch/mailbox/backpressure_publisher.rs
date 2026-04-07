use core::marker::PhantomData;

use fraktor_utils_rs::core::sync::ArcShared;

use super::metrics_event::MailboxPressureEvent;

type BackpressureCallback = dyn Fn(&MailboxPressureEvent) + Send + Sync + 'static;

/// Publishes mailbox pressure notifications to interested runtime components.
#[derive(Clone)]
pub struct BackpressurePublisher {
  callback: ArcShared<BackpressureCallback>,
  _marker:  PhantomData<()>,
}

impl BackpressurePublisher {
  /// Creates a publisher from a shared callback.
  #[must_use]
  pub fn new(callback: ArcShared<BackpressureCallback>) -> Self {
    Self { callback, _marker: PhantomData }
  }

  /// Creates a publisher from a closure without requiring manual trait-object erasure.
  #[must_use]
  pub fn from_fn<F>(callback: F) -> Self
  where
    F: Fn(&MailboxPressureEvent) + Send + Sync + 'static, {
    Self::new(ArcShared::new(callback))
  }

  /// Publishes a pressure event to the configured target.
  pub fn publish(&self, event: &MailboxPressureEvent) {
    (self.callback)(event);
  }
}
