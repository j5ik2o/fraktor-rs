use alloc::{boxed::Box, format, string::String};
use core::marker::PhantomData;

use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::ActorRef,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  serialization::{SerializationCallScope, SerializedMessage, default_serialization_extension_id},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::SharedAccess;

use super::{StreamRefEndpointSlot, StreamRefHandoff, stream_ref_protocol::StreamRefProtocol};
use crate::{
  DemandTracker, DynValue, SinkDecision, SinkLogic, StreamError, downcast_value,
  materialization::{StreamDone, StreamFuture},
  stage::{StageActor, StageActorEnvelope, StageActorReceive},
  stream_ref::{
    StreamRefAck, StreamRefCumulativeDemand, StreamRefOnSubscribeHandshake, StreamRefRemoteStreamCompleted,
    StreamRefRemoteStreamFailure, StreamRefSequencedOnNext, StreamRefSettings,
  },
};

#[cfg(test)]
#[path = "stream_ref_sink_logic_test.rs"]
mod tests;

enum StreamRefSinkSubscription {
  AwaitingRemote,
  Subscribed,
}

/// Sink logic backed by a local stream-reference handoff.
pub(crate) struct StreamRefSinkLogic<T> {
  handoff:         StreamRefHandoff<T>,
  endpoint:        Option<StreamRefEndpointSlot>,
  subscription:    StreamRefSinkSubscription,
  completion:      Option<StreamFuture<StreamDone>>,
  settings:        StreamRefSettings,
  demand_started:  bool,
  terminal_queued: bool,
  waiting_ticks:   u64,
  _pd:             PhantomData<fn(T)>,
}

impl<T> StreamRefSinkLogic<T> {
  pub(crate) fn awaiting_remote_subscription(handoff: StreamRefHandoff<T>) -> Self {
    Self::new(handoff, None, StreamRefSinkSubscription::AwaitingRemote, None)
  }

  pub(crate) fn awaiting_remote_subscription_with_endpoint(
    handoff: StreamRefHandoff<T>,
    endpoint: StreamRefEndpointSlot,
  ) -> Self {
    let mut logic = Self::awaiting_remote_subscription(handoff);
    logic.endpoint = Some(endpoint);
    logic
  }

  pub(crate) fn subscribed(handoff: StreamRefHandoff<T>, completion: Option<StreamFuture<StreamDone>>) -> Self {
    Self::new(handoff, None, StreamRefSinkSubscription::Subscribed, completion)
  }

  fn new(
    handoff: StreamRefHandoff<T>,
    endpoint: Option<StreamRefEndpointSlot>,
    subscription: StreamRefSinkSubscription,
    completion: Option<StreamFuture<StreamDone>>,
  ) -> Self {
    Self {
      handoff,
      endpoint,
      subscription,
      completion,
      settings: StreamRefSettings::new(),
      demand_started: false,
      terminal_queued: false,
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

  fn queue_completion_once(&mut self) {
    if self.terminal_queued {
      return;
    }
    self.handoff.complete();
    self.terminal_queued = true;
  }

  fn attach_source_ref_endpoint_actor(&mut self, system: &ActorSystem)
  where
    T: Send + Sync + 'static, {
    let Some(endpoint) = &self.endpoint else {
      return;
    };
    if endpoint.actor_ref().is_ok() {
      return;
    }
    let endpoint_actor = StageActor::new(system, Box::new(StreamRefTargetNotInitializedReceive));
    endpoint_actor.r#become(Box::new(StreamRefEndpointReceive::<T>::new(
      self.handoff.clone(),
      system.clone(),
      endpoint_actor.actor_ref().clone(),
    )));
    endpoint.set_actor_ref(endpoint_actor.actor_ref().clone());
    self.handoff.attach_endpoint_actor(endpoint_actor, None);
  }
}

struct StreamRefTargetNotInitializedReceive;

impl StageActorReceive for StreamRefTargetNotInitializedReceive {
  fn receive(&mut self, _envelope: StageActorEnvelope) -> Result<(), StreamError> {
    Err(StreamError::StreamRefTargetNotInitialized)
  }
}

struct StreamRefEndpointReceive<T> {
  handoff:            StreamRefHandoff<T>,
  system:             ActorSystem,
  endpoint_actor_ref: ActorRef,
  partner_actor:      Option<ActorRef>,
  _pd:                PhantomData<fn(T)>,
}

impl<T> StreamRefEndpointReceive<T>
where
  T: Send + Sync + 'static,
{
  const fn new(handoff: StreamRefHandoff<T>, system: ActorSystem, endpoint_actor_ref: ActorRef) -> Self {
    Self { handoff, system, endpoint_actor_ref, partner_actor: None, _pd: PhantomData }
  }

  fn actor_key(actor_ref: &ActorRef) -> Result<String, StreamError> {
    actor_ref.canonical_path().map(|path| path.to_canonical_uri()).ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  fn stream_error_from_context(message: impl Into<String>) -> StreamError {
    StreamError::failed_with_context(message.into())
  }

  fn serialize_value(&self, value: &T) -> Result<SerializedMessage, StreamError> {
    let extension = self.system.extended().register_extension(&default_serialization_extension_id());
    extension
      .with_read(|serialization| serialization.serialize(value, SerializationCallScope::Remote))
      .map_err(|error| Self::stream_error_from_context(format!("StreamRef payload serialization failed: {error:?}")))
  }

  fn send_to_partner<M>(&mut self, message: M) -> Result<(), StreamError>
  where
    M: Send + Sync + 'static, {
    let Some(partner_actor) = &self.partner_actor else {
      return Err(StreamError::StreamRefTargetNotInitialized);
    };
    let mut partner_actor = partner_actor.clone();
    partner_actor
      .try_tell(AnyMessage::new(message).with_sender(self.endpoint_actor_ref.clone()))
      .map_err(|error| StreamError::from_send_error(&error))
  }

  fn accept_handshake(
    &mut self,
    message: &StreamRefOnSubscribeHandshake,
    sender: &ActorRef,
  ) -> Result<(), StreamError> {
    let partner_actor = sender.clone();
    self.handoff.pair_partner_actor(String::from(message.target_ref_path()), partner_actor.clone())?;
    self.partner_actor = Some(partner_actor);
    self.send_to_partner(StreamRefAck)
  }

  fn accept_demand(&mut self, message: StreamRefCumulativeDemand, sender: &ActorRef) -> Result<(), StreamError> {
    let sender_key = Self::actor_key(sender)?;
    self.handoff.ensure_partner(sender_key)?;
    self.handoff.record_cumulative_demand_from(message.seq_nr(), message.demand())?;
    self.flush_ready_protocols()
  }

  fn flush_ready_protocols(&mut self) -> Result<(), StreamError> {
    let messages = self.handoff.drain_ready_protocols()?;
    let mut terminal_drained = false;
    for message in messages {
      match message {
        | StreamRefProtocol::SequencedOnNext { seq_nr, payload } => {
          let value = downcast_value::<T>(payload)?;
          let serialized = self.serialize_value(&value)?;
          self.send_to_partner(StreamRefSequencedOnNext::new(seq_nr, serialized))?;
        },
        | StreamRefProtocol::RemoteStreamCompleted { seq_nr } => {
          terminal_drained = true;
          self.send_to_partner(StreamRefRemoteStreamCompleted::new(seq_nr))?;
        },
        | StreamRefProtocol::RemoteStreamFailure { message } => {
          terminal_drained = true;
          self.send_to_partner(StreamRefRemoteStreamFailure::new(message.into_owned()))?;
        },
        | StreamRefProtocol::CumulativeDemand { .. }
        | StreamRefProtocol::OnSubscribeHandshake
        | StreamRefProtocol::Ack => {
          return Err(StreamError::Failed);
        },
      }
    }
    if terminal_drained {
      self.handoff.cleanup_after_terminal_delivery()?;
    }
    Ok(())
  }

  fn accept_partner_terminated(&self, terminated: &Pid) -> Result<(), StreamError> {
    if self.handoff.is_terminal() {
      return Ok(());
    }
    let error = StreamError::RemoteStreamRefActorTerminated {
      message: format!("remote stream ref partner actor terminated: {terminated:?}").into(),
    };
    Err(self.handoff.fail_and_report(error))
  }
}

impl<T> StageActorReceive for StreamRefEndpointReceive<T>
where
  T: Send + Sync + 'static,
{
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    if let Some(SystemMessage::DeathWatchNotification(terminated)) = envelope.message().downcast_ref::<SystemMessage>()
    {
      return self.accept_partner_terminated(terminated);
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefOnSubscribeHandshake>() {
      return self.accept_handshake(message, envelope.sender());
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefCumulativeDemand>() {
      return self.accept_demand(*message, envelope.sender());
    }
    Err(StreamError::Failed)
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
    self.handoff.drain_endpoint_actor()?;
    let _started = self.start_demand_if_subscribed(demand)?;
    Ok(())
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    self.handoff.drain_endpoint_actor()?;
    if !self.handoff.is_subscribed() {
      return Err(StreamError::WouldBlock);
    }
    let value = downcast_value::<T>(input)?;
    self.handoff.offer(value)?;
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.queue_completion_once();
    self.complete_materialized(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.handoff.fail(error.clone());
    self.complete_materialized(Err(error));
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    self.handoff.drain_endpoint_actor()?;
    if matches!(self.subscription, StreamRefSinkSubscription::AwaitingRemote) {
      self.await_subscription()?;
    }
    self.start_demand_if_subscribed(demand)
  }

  fn on_upstream_finish(&mut self) -> Result<bool, StreamError> {
    self.queue_completion_once();
    Ok(true)
  }

  fn has_pending_work(&self) -> bool {
    self.terminal_queued && self.handoff.has_pending_protocols()
  }

  fn attach_stream_ref_settings(&mut self, settings: StreamRefSettings) {
    self.handoff.configure_buffer_capacity(settings.buffer_capacity());
    self.settings = settings;
  }

  fn attach_actor_system(&mut self, system: ActorSystem) {
    self.attach_source_ref_endpoint_actor(&system);
  }
}
