use core::marker::PhantomData;

use super::StreamRefHandoff;
use crate::{
  DemandTracker, DynValue, SinkDecision, SinkLogic, StreamError, downcast_value,
  materialization::{StreamDone, StreamFuture},
  stream_ref::StreamRefSettings,
};

#[cfg(test)]
mod tests;

enum StreamRefSinkSubscription {
  AwaitingRemote,
  Subscribed,
}

/// Sink logic backed by a local stream-reference handoff.
pub(crate) struct StreamRefSinkLogic<T> {
  handoff:        StreamRefHandoff<T>,
  subscription:   StreamRefSinkSubscription,
  completion:     Option<StreamFuture<StreamDone>>,
  settings:       StreamRefSettings,
  demand_started: bool,
  waiting_ticks:  u64,
  _pd:            PhantomData<fn(T)>,
}

impl<T> StreamRefSinkLogic<T> {
  pub(crate) fn awaiting_remote_subscription(handoff: StreamRefHandoff<T>) -> Self {
    Self::new(handoff, StreamRefSinkSubscription::AwaitingRemote, None)
  }

  pub(crate) fn subscribed(handoff: StreamRefHandoff<T>, completion: Option<StreamFuture<StreamDone>>) -> Self {
    Self::new(handoff, StreamRefSinkSubscription::Subscribed, completion)
  }

  fn new(
    handoff: StreamRefHandoff<T>,
    subscription: StreamRefSinkSubscription,
    completion: Option<StreamFuture<StreamDone>>,
  ) -> Self {
    Self {
      handoff,
      subscription,
      completion,
      settings: StreamRefSettings::new(),
      demand_started: false,
      waiting_ticks: 0,
      _pd: PhantomData,
    }
  }

  fn start_demand_if_subscribed(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    if self.demand_started {
      return Ok(false);
    }
    if !self.handoff.is_subscribed() {
      return Ok(false);
    }
    demand.request(1)?;
    self.demand_started = true;
    Ok(true)
  }

  fn await_subscription(&mut self) -> Result<(), StreamError> {
    if self.handoff.is_subscribed() {
      return Ok(());
    }
    self.waiting_ticks = self.waiting_ticks.saturating_add(1);
    if self.waiting_ticks >= u64::from(self.settings.subscription_timeout_ticks()) {
      return Err(StreamRefHandoff::<T>::subscription_timeout_error());
    }
    Ok(())
  }

  fn complete_materialized(&self, result: Result<StreamDone, StreamError>) {
    if let Some(completion) = &self.completion {
      completion.complete(result);
    }
  }
}

impl<T> SinkLogic for StreamRefSinkLogic<T>
where
  T: Send + Sync + 'static,
{
  fn can_accept_input(&self) -> bool {
    self.handoff.is_subscribed()
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    let _started = self.start_demand_if_subscribed(demand)?;
    Ok(())
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<T>(input)?;
    self.handoff.offer(value)?;
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.handoff.complete();
    self.complete_materialized(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.handoff.fail(error.clone());
    self.complete_materialized(Err(error));
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    if matches!(self.subscription, StreamRefSinkSubscription::AwaitingRemote) {
      self.await_subscription()?;
    }
    self.start_demand_if_subscribed(demand)
  }

  fn attach_stream_ref_settings(&mut self, settings: StreamRefSettings) {
    self.handoff.configure_buffer_capacity(settings.buffer_capacity());
    self.settings = settings;
  }
}
