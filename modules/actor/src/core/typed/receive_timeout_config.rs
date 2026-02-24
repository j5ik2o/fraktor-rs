//! Internal configuration state for actor receive timeouts.

use alloc::boxed::Box;
use core::{marker::PhantomData, time::Duration};

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::scheduler::SchedulerHandle;

/// Stores the receive timeout configuration for a single actor.
///
/// This is held by `TypedActorAdapter` and exposed to the typed context
/// via a mutable pointer so that `set_receive_timeout` can modify it.
pub(crate) struct ReceiveTimeoutConfig<M, TB = NoStdToolbox>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  pub(crate) duration:        Duration,
  pub(crate) message_factory: Box<dyn Fn() -> M + Send + Sync>,
  pub(crate) handle:          Option<SchedulerHandle>,
  _marker:                    PhantomData<TB>,
}

impl<M, TB> ReceiveTimeoutConfig<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new receive timeout configuration.
  pub(crate) fn new<F>(duration: Duration, message_factory: F) -> Self
  where
    F: Fn() -> M + Send + Sync + 'static, {
    Self { duration, message_factory: Box::new(message_factory), handle: None, _marker: PhantomData }
  }

  /// Produces the timeout message.
  pub(crate) fn make_message(&self) -> M {
    (self.message_factory)()
  }
}
