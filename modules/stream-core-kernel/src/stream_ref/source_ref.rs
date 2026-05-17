#[cfg(test)]
#[path = "source_ref_test.rs"]
mod tests;

use alloc::{boxed::Box, format, string::String};
use core::{any::TypeId, marker::PhantomData, num::NonZeroU64};

use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::ActorRef,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  serialization::default_serialization_extension_id,
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::SharedAccess;

use crate::{
  DynValue, SourceLogic, StreamError,
  dsl::Source,
  r#impl::streamref::{StreamRefEndpointSlot, StreamRefHandoff, StreamRefSourceLogic},
  materialization::StreamNotUsed,
  stage::{StageActor, StageActorEnvelope, StageActorReceive, StageKind},
  stream_ref::{
    StreamRefAck, StreamRefCumulativeDemand, StreamRefOnSubscribeHandshake, StreamRefRemoteStreamCompleted,
    StreamRefRemoteStreamFailure, StreamRefSequencedOnNext, StreamRefSettings,
  },
};

/// Reference to a source side of a stream reference.
pub struct SourceRef<T> {
  backend: SourceRefBackend<T>,
  _pd:     PhantomData<fn() -> T>,
}

enum SourceRefBackend<T> {
  Local { handoff: StreamRefHandoff<T>, endpoint: StreamRefEndpointSlot },
  ActorBacked { endpoint: StreamRefEndpointSlot },
}

impl<T> SourceRef<T> {
  pub(crate) fn new(handoff: StreamRefHandoff<T>, endpoint: StreamRefEndpointSlot) -> Self {
    Self { backend: SourceRefBackend::Local { handoff, endpoint }, _pd: PhantomData }
  }

  pub(crate) fn from_endpoint_actor(actor_ref: ActorRef) -> Self {
    Self {
      backend: SourceRefBackend::ActorBacked { endpoint: StreamRefEndpointSlot::from_actor_ref(actor_ref) },
      _pd:     PhantomData,
    }
  }

  pub(crate) fn canonical_actor_path(&self) -> Result<String, StreamError> {
    self.endpoint().canonical_actor_path()
  }

  pub(crate) fn endpoint_actor_ref(&self) -> Result<ActorRef, StreamError> {
    self.endpoint().actor_ref()
  }

  const fn endpoint(&self) -> &StreamRefEndpointSlot {
    match &self.backend {
      | SourceRefBackend::Local { endpoint, .. } | SourceRefBackend::ActorBacked { endpoint } => endpoint,
    }
  }
}

impl<T> SourceRef<T>
where
  T: Send + Sync + 'static,
{
  /// Converts this reference into the source it points to.
  #[must_use]
  pub fn into_source(self) -> Source<T, StreamNotUsed> {
    match self.backend {
      | SourceRefBackend::Local { handoff, endpoint } => {
        if endpoint.actor_ref().is_ok() {
          debug_assert!(endpoint.canonical_actor_path().is_ok());
        }
        handoff.subscribe();
        Source::from_logic(StageKind::Custom, StreamRefSourceLogic::subscribed(handoff))
      },
      | SourceRefBackend::ActorBacked { endpoint } => match endpoint.actor_ref() {
        | Ok(actor_ref) => {
          debug_assert!(actor_ref.canonical_path().is_some());
          Source::from_logic(StageKind::Custom, ActorBackedSourceRefLogic::<T>::new(actor_ref))
        },
        | Err(error) => Source::failed(error),
      },
    }
  }
}

struct ActorBackedSourceRefLogic<T> {
  target_actor:   ActorRef,
  handoff:        StreamRefHandoff<T>,
  endpoint_actor: Option<StageActor>,
  startup_error:  Option<StreamError>,
  waiting_ticks:  u64,
  settings:       StreamRefSettings,
  _pd:            PhantomData<fn() -> T>,
}

impl<T> ActorBackedSourceRefLogic<T> {
  fn new(target_actor: ActorRef) -> Self {
    Self {
      target_actor,
      handoff: StreamRefHandoff::new(),
      endpoint_actor: None,
      startup_error: None,
      waiting_ticks: 0,
      settings: StreamRefSettings::new(),
      _pd: PhantomData,
    }
  }

  fn actor_key(actor_ref: &ActorRef) -> Result<String, StreamError> {
    actor_ref.canonical_path().map(|path| path.to_canonical_uri()).ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  fn set_startup_result(&mut self, result: Result<(), StreamError>) {
    if let Err(error) = result
      && self.startup_error.is_none()
    {
      self.startup_error = Some(error);
    }
  }

  fn drain_endpoint_actor(&self) -> Result<(), StreamError> {
    match &self.endpoint_actor {
      | Some(endpoint_actor) => endpoint_actor.drain_pending(),
      | None => Ok(()),
    }
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

  fn endpoint_actor_ref(&self) -> Result<ActorRef, StreamError> {
    self
      .endpoint_actor
      .as_ref()
      .map(|endpoint_actor| endpoint_actor.actor_ref().clone())
      .ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  fn send_handshake(&mut self) -> Result<(), StreamError> {
    let endpoint_actor_ref = self.endpoint_actor_ref()?;
    let target_ref_path = Self::actor_key(&endpoint_actor_ref)?;
    let message = StreamRefOnSubscribeHandshake::new(target_ref_path);
    let mut target_actor = self.target_actor.clone();
    target_actor
      .try_tell(AnyMessage::new(message).with_sender(endpoint_actor_ref))
      .map_err(|error| StreamError::from_send_error(&error))
  }

  fn send_demand(&mut self) -> Result<(), StreamError> {
    let demand = NonZeroU64::MIN;
    let endpoint_actor_ref = self.endpoint_actor_ref()?;
    let message = StreamRefCumulativeDemand::new(self.handoff.next_expected_seq_nr(), demand);
    let mut target_actor = self.target_actor.clone();
    target_actor
      .try_tell(AnyMessage::new(message).with_sender(endpoint_actor_ref))
      .map_err(|error| StreamError::from_send_error(&error))?;
    self.handoff.record_cumulative_demand()
  }

  fn watch_target_actor(&self) -> Result<(), StreamError> {
    let Some(endpoint_actor) = &self.endpoint_actor else {
      return Err(StreamError::StreamRefTargetNotInitialized);
    };
    endpoint_actor.watch(&self.target_actor)
  }
}

impl<T> SourceLogic for ActorBackedSourceRefLogic<T>
where
  T: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if let Some(error) = &self.startup_error {
      return Err(error.clone());
    }
    self.drain_endpoint_actor()?;
    self.await_subscription()?;
    match self.handoff.poll_or_drain() {
      | Ok(value) => return Ok(value.map(|value| Box::new(value) as DynValue)),
      | Err(StreamError::WouldBlock) => {},
      | Err(error) => return Err(error),
    }
    self.send_demand()?;
    self.drain_endpoint_actor()?;
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
    let endpoint_actor = StageActor::new(
      &system,
      Box::new(ActorBackedSourceRefReceive::<T>::new(self.handoff.clone(), system.clone(), &self.target_actor)),
    );
    self.handoff.attach_endpoint_actor(endpoint_actor.clone(), Some(self.target_actor.clone()));
    self.endpoint_actor = Some(endpoint_actor);
    let watch_result = self.watch_target_actor();
    self.set_startup_result(watch_result);
    if self.startup_error.is_some() {
      return;
    }
    let handshake_result = self.send_handshake();
    self.set_startup_result(handshake_result);
  }
}

struct ActorBackedSourceRefReceive<T> {
  handoff:          StreamRefHandoff<T>,
  system:           ActorSystem,
  target_actor_pid: Pid,
  target_actor_ref: String,
  _pd:              PhantomData<fn() -> T>,
}

impl<T> ActorBackedSourceRefReceive<T> {
  fn new(handoff: StreamRefHandoff<T>, system: ActorSystem, target_actor: &ActorRef) -> Self {
    Self {
      handoff,
      system,
      target_actor_pid: target_actor.pid(),
      target_actor_ref: Self::actor_label(target_actor),
      _pd: PhantomData,
    }
  }

  fn stream_error_from_context(message: impl Into<String>) -> StreamError {
    StreamError::failed_with_context(message.into())
  }

  fn actor_label(actor_ref: &ActorRef) -> String {
    actor_ref.canonical_path().map(|path| path.to_canonical_uri()).unwrap_or_else(|| format!("{:?}", actor_ref.pid()))
  }

  fn ensure_sender(&self, sender: &ActorRef) -> Result<String, StreamError> {
    let got_ref = Self::actor_label(sender);
    // remoteではcanonical path、local loopbackではPidだけが安定するため両方を許可する。
    if got_ref == self.target_actor_ref || sender.pid() == self.target_actor_pid {
      return Ok(got_ref);
    }
    Err(StreamError::InvalidPartnerActor {
      expected_ref: self.target_actor_ref.clone().into(),
      got_ref:      got_ref.into(),
      message:      "stream ref message came from a non-partner actor".into(),
    })
  }

  fn deserialize_value(&self, message: &StreamRefSequencedOnNext) -> Result<T, StreamError>
  where
    T: Send + Sync + 'static, {
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

impl<T> StageActorReceive for ActorBackedSourceRefReceive<T>
where
  T: Send + Sync + 'static,
{
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    if let Some(SystemMessage::DeathWatchNotification(terminated)) = envelope.message().downcast_ref::<SystemMessage>()
    {
      return self.accept_partner_terminated(terminated);
    }
    if envelope.message().downcast_ref::<StreamRefAck>().is_some() {
      let got_ref = self.ensure_sender(envelope.sender())?;
      self.handoff.pair_partner_actor(got_ref, envelope.sender().clone())?;
      return Ok(());
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefSequencedOnNext>() {
      self.ensure_sender(envelope.sender())?;
      let value = self.deserialize_value(message)?;
      return self.handoff.enqueue_remote_element(message.seq_nr(), value);
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefRemoteStreamCompleted>() {
      self.ensure_sender(envelope.sender())?;
      return self.handoff.enqueue_remote_completed(message.seq_nr());
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefRemoteStreamFailure>() {
      self.ensure_sender(envelope.sender())?;
      self.handoff.enqueue_remote_failure(String::from(message.message()));
      return Ok(());
    }
    Err(StreamError::Failed)
  }
}
