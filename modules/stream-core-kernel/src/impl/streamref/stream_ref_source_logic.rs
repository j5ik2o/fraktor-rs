use alloc::{boxed::Box, format, string::String};
use core::{any::TypeId, marker::PhantomData, num::NonZeroU64};

use fraktor_actor_core_kernel_rs::{
  actor::{actor_ref::ActorRef, messaging::system_message::SystemMessage},
  serialization::default_serialization_extension_id,
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::SharedAccess;

use super::{
  StreamRefEndpointReceiveState, StreamRefEndpointSlot, StreamRefHandoff, StreamRefTargetNotInitializedReceive,
};
use crate::{
  DynValue, SourceLogic, StreamError,
  stage::{StageActor, StageActorEnvelope, StageActorReceive},
  stream_ref::{
    StreamRefOnSubscribeHandshake, StreamRefRemoteStreamCompleted, StreamRefRemoteStreamFailure,
    StreamRefSequencedOnNext, StreamRefSettings,
  },
};

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
  endpoint:      Option<StreamRefEndpointSlot>,
  subscription:  StreamRefSourceSubscription,
  settings:      StreamRefSettings,
  waiting_ticks: u64,
  _pd:           PhantomData<fn() -> T>,
}

impl<T> StreamRefSourceLogic<T> {
  pub(crate) fn awaiting_remote_subscription(handoff: StreamRefHandoff<T>) -> Self {
    Self::new(handoff, None, StreamRefSourceSubscription::AwaitingRemote)
  }

  pub(crate) fn awaiting_remote_subscription_with_endpoint(
    handoff: StreamRefHandoff<T>,
    endpoint: StreamRefEndpointSlot,
  ) -> Self {
    let mut logic = Self::awaiting_remote_subscription(handoff);
    logic.endpoint = Some(endpoint);
    logic
  }

  pub(crate) fn subscribed(handoff: StreamRefHandoff<T>) -> Self {
    Self::new(handoff, None, StreamRefSourceSubscription::Subscribed)
  }

  fn new(
    handoff: StreamRefHandoff<T>,
    endpoint: Option<StreamRefEndpointSlot>,
    subscription: StreamRefSourceSubscription,
  ) -> Self {
    Self { handoff, endpoint, subscription, settings: StreamRefSettings::new(), waiting_ticks: 0, _pd: PhantomData }
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

  fn attach_sink_ref_endpoint_actor(&mut self, system: &ActorSystem)
  where
    T: Send + Sync + 'static, {
    let Some(endpoint) = &self.endpoint else {
      return;
    };
    if endpoint.actor_ref().is_ok() {
      return;
    }
    let endpoint_actor = StageActor::new(system, Box::new(StreamRefTargetNotInitializedReceive));
    endpoint_actor.r#become(Box::new(SourceRefEndpointReceive::<T>::new(
      self.handoff.clone(),
      system.clone(),
      endpoint_actor.actor_ref().clone(),
    )));
    endpoint.set_actor_ref(endpoint_actor.actor_ref().clone());
    self.handoff.attach_endpoint_actor(endpoint_actor, None);
  }

  fn signal_partner_demand(&self) -> Result<(), StreamError> {
    let demand = NonZeroU64::MIN;
    self.handoff.send_cumulative_demand_to_partner(self.handoff.next_expected_seq_nr(), demand)?;
    self.handoff.record_cumulative_demand()
  }
}

struct SourceRefEndpointReceive<T> {
  endpoint: StreamRefEndpointReceiveState<T>,
  system:   ActorSystem,
}

impl<T> SourceRefEndpointReceive<T>
where
  T: Send + Sync + 'static,
{
  const fn new(handoff: StreamRefHandoff<T>, system: ActorSystem, endpoint_actor_ref: ActorRef) -> Self {
    Self { endpoint: StreamRefEndpointReceiveState::new(handoff, endpoint_actor_ref), system }
  }

  fn stream_error_from_context(message: impl Into<String>) -> StreamError {
    StreamRefEndpointReceiveState::<T>::stream_error_from_context(message)
  }

  fn deserialize_value(&self, message: &StreamRefSequencedOnNext) -> Result<T, StreamError> {
    let extension = self.system.extended().register_extension(&default_serialization_extension_id());
    let payload = extension
      .with_read(|serialization| serialization.deserialize(message.payload(), Some(TypeId::of::<T>())))
      .map_err(|error| {
        Self::stream_error_from_context(format!("StreamRef payload deserialization failed: {error:?}"))
      })?;
    payload
      .downcast::<T>()
      .map(|value| *value)
      .map_err(|_| Self::stream_error_from_context("StreamRef payload type mismatch"))
  }

  #[cfg(test)]
  fn send_to_partner<M>(&mut self, message: M) -> Result<(), StreamError>
  where
    M: Send + Sync + 'static, {
    self.endpoint.send_to_partner(message)
  }

  fn accept_handshake(
    &mut self,
    message: &StreamRefOnSubscribeHandshake,
    sender: &ActorRef,
  ) -> Result<(), StreamError> {
    self.endpoint.accept_handshake(message, sender)
  }

  fn ensure_sender(&self, sender: &ActorRef) -> Result<(), StreamError> {
    self.endpoint.ensure_sender(sender)
  }
}

impl<T> StageActorReceive for SourceRefEndpointReceive<T>
where
  T: Send + Sync + 'static,
{
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    if let Some(SystemMessage::DeathWatchNotification(terminated)) = envelope.message().downcast_ref::<SystemMessage>()
    {
      return self.endpoint.accept_partner_terminated(terminated);
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefOnSubscribeHandshake>() {
      return self.accept_handshake(message, envelope.sender());
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefSequencedOnNext>() {
      self.ensure_sender(envelope.sender())?;
      let value = self.deserialize_value(message)?;
      return self.endpoint.handoff().enqueue_remote_element(message.seq_nr(), value);
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefRemoteStreamCompleted>() {
      self.ensure_sender(envelope.sender())?;
      return self.endpoint.handoff().enqueue_remote_completed(message.seq_nr());
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefRemoteStreamFailure>() {
      self.ensure_sender(envelope.sender())?;
      self.endpoint.handoff().enqueue_remote_failure(String::from(message.message()));
      return Ok(());
    }
    Err(StreamError::Failed)
  }
}

impl<T> SourceLogic for StreamRefSourceLogic<T>
where
  T: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    self.handoff.drain_endpoint_actor()?;
    if matches!(self.subscription, StreamRefSourceSubscription::AwaitingRemote) {
      self.await_subscription()?;
    }
    match self.handoff.poll_or_drain() {
      | Ok(value) => return Ok(value.map(|value| Box::new(value) as DynValue)),
      | Err(StreamError::WouldBlock) => {},
      | Err(error) => return Err(error),
    }
    self.signal_partner_demand()?;
    self.handoff.drain_endpoint_actor()?;
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

  fn attach_actor_system(&mut self, system: ActorSystem) {
    self.attach_sink_ref_endpoint_actor(&system);
  }
}
