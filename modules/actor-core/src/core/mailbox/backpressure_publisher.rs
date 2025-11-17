use core::marker::PhantomData;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use super::MailboxPressureEvent;
use crate::core::dispatcher::DispatcherGeneric;

type BackpressureCallback = dyn Fn(&MailboxPressureEvent) + Send + Sync + 'static;

/// Publishes mailbox pressure notifications to interested runtime components.
#[derive(Clone)]
pub struct BackpressurePublisherGeneric<TB: RuntimeToolbox + 'static> {
  callback: ArcShared<BackpressureCallback>,
  _marker:  PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> BackpressurePublisherGeneric<TB> {
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

  /// Creates a publisher that forwards events to the dispatcher.
  #[must_use]
  pub fn from_dispatcher(dispatcher: DispatcherGeneric<TB>) -> Self {
    let publisher = move |event: &MailboxPressureEvent| {
      dispatcher.notify_backpressure(event);
    };
    Self::from_fn(publisher)
  }

  /// Publishes a pressure event to the configured target.
  pub fn publish(&self, event: &MailboxPressureEvent) {
    (self.callback)(event);
  }
}
