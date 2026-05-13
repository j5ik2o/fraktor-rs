use alloc::boxed::Box;
use core::marker::PhantomData;

use super::StreamRefHandoff;
use crate::{DynValue, SourceLogic, StreamError, stream_ref::StreamRefSettings};

#[cfg(test)]
#[path = "stream_ref_source_logic_test.rs"]
mod tests;

enum StreamRefSourceSubscription {
  AwaitingRemote,
  Subscribed,
}

/// Source logic backed by a local stream-reference handoff.
pub(crate) struct StreamRefSourceLogic<T> {
  handoff:       StreamRefHandoff<T>,
  subscription:  StreamRefSourceSubscription,
  settings:      StreamRefSettings,
  waiting_ticks: u64,
  _pd:           PhantomData<fn() -> T>,
}

impl<T> StreamRefSourceLogic<T> {
  pub(crate) fn awaiting_remote_subscription(handoff: StreamRefHandoff<T>) -> Self {
    Self::new(handoff, StreamRefSourceSubscription::AwaitingRemote)
  }

  pub(crate) fn subscribed(handoff: StreamRefHandoff<T>) -> Self {
    Self::new(handoff, StreamRefSourceSubscription::Subscribed)
  }

  fn new(handoff: StreamRefHandoff<T>, subscription: StreamRefSourceSubscription) -> Self {
    Self { handoff, subscription, settings: StreamRefSettings::new(), waiting_ticks: 0, _pd: PhantomData }
  }

  fn await_subscription(&mut self) -> Result<(), StreamError> {
    if self.handoff.is_subscribed() {
      return Ok(());
    }
    self.waiting_ticks = self.waiting_ticks.saturating_add(1);
    if self.waiting_ticks >= u64::from(self.settings.subscription_timeout_ticks()) {
      return Err(StreamRefHandoff::<T>::subscription_timeout_error());
    }
    Err(StreamError::WouldBlock)
  }
}

impl<T> SourceLogic for StreamRefSourceLogic<T>
where
  T: Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if matches!(self.subscription, StreamRefSourceSubscription::AwaitingRemote) {
      self.await_subscription()?;
    }
    self.handoff.record_cumulative_demand()?;
    self.handoff.poll_or_drain().map(|value| value.map(|value| Box::new(value) as DynValue))
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.handoff.close_for_cancel();
    Ok(())
  }

  fn should_drain_on_shutdown(&self) -> bool {
    false
  }

  fn attach_stream_ref_settings(&mut self, settings: StreamRefSettings) {
    self.handoff.configure_buffer_capacity(settings.buffer_capacity());
    self.settings = settings;
  }
}
