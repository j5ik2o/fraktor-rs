#[cfg(test)]
#[path = "sink_ref_test.rs"]
mod tests;

use alloc::{borrow::Cow, boxed::Box, format, string::String};
use core::{marker::PhantomData, num::NonZeroU64};

use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::ActorRef,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  serialization::{SerializationCallScope, SerializedMessage, default_serialization_extension_id},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use crate::{
  DemandTracker, DynValue, SinkDecision, SinkLogic, StreamError, downcast_value,
  dsl::Sink,
  r#impl::streamref::{StreamRefEndpointSlot, StreamRefHandoff, StreamRefSinkLogic},
  materialization::StreamNotUsed,
  stage::{StageActor, StageActorEnvelope, StageActorReceive, StageKind},
  stream_ref::{
    StreamRefAck, StreamRefCumulativeDemand, StreamRefOnSubscribeHandshake, StreamRefRemoteStreamCompleted,
    StreamRefRemoteStreamFailure, StreamRefSequencedOnNext,
  },
};

/// Reference to a sink side of a stream reference.
pub struct SinkRef<T> {
  backend: SinkRefBackend<T>,
  _pd:     PhantomData<fn(T)>,
}

enum SinkRefBackend<T> {
  Local { handoff: StreamRefHandoff<T>, endpoint: StreamRefEndpointSlot },
  ActorBacked { endpoint: StreamRefEndpointSlot },
}

impl<T> SinkRef<T> {
  pub(crate) fn new(handoff: StreamRefHandoff<T>, endpoint: StreamRefEndpointSlot) -> Self {
    Self { backend: SinkRefBackend::Local { handoff, endpoint }, _pd: PhantomData }
  }

  pub(crate) fn from_endpoint_actor(actor_ref: ActorRef) -> Self {
    Self {
      backend: SinkRefBackend::ActorBacked { endpoint: StreamRefEndpointSlot::from_actor_ref(actor_ref) },
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
      | SinkRefBackend::Local { endpoint, .. } | SinkRefBackend::ActorBacked { endpoint } => endpoint,
    }
  }
}

impl<T> SinkRef<T>
where
  T: Send + Sync + 'static,
{
  /// Converts this reference into the sink it points to.
  #[must_use]
  pub fn into_sink(self) -> Sink<T, StreamNotUsed> {
    match self.backend {
      | SinkRefBackend::Local { handoff, endpoint } => {
        if endpoint.actor_ref().is_ok() {
          debug_assert!(endpoint.canonical_actor_path().is_ok());
        }
        handoff.subscribe();
        let logic = StreamRefSinkLogic::subscribed(handoff, None);
        Sink::from_logic(StageKind::Custom, logic)
      },
      | SinkRefBackend::ActorBacked { endpoint } => {
        debug_assert!(endpoint.canonical_actor_path().is_ok());
        match endpoint.actor_ref() {
          | Ok(actor_ref) => Sink::from_logic(StageKind::Custom, ActorBackedSinkRefLogic::<T>::new(actor_ref)),
          | Err(error) => Sink::from_logic(StageKind::Custom, ActorBackedSinkRefLogic::<T>::failed(error)),
        }
      },
    }
  }
}

struct ActorBackedSinkRefLogic<T> {
  target_actor:   Option<ActorRef>,
  endpoint_actor: Option<StageActor>,
  system:         Option<ActorSystem>,
  state:          ActorBackedSinkRefStateShared,
  startup_error:  Option<StreamError>,
  _pd:            PhantomData<fn(T)>,
}

impl<T> ActorBackedSinkRefLogic<T> {
  fn new(target_actor: ActorRef) -> Self {
    Self {
      target_actor:   Some(target_actor),
      endpoint_actor: None,
      system:         None,
      state:          ActorBackedSinkRefStateShared::new(),
      startup_error:  None,
      _pd:            PhantomData,
    }
  }

  fn failed(error: StreamError) -> Self {
    Self {
      target_actor:   None,
      endpoint_actor: None,
      system:         None,
      state:          ActorBackedSinkRefStateShared::new(),
      startup_error:  Some(error),
      _pd:            PhantomData,
    }
  }

  fn actor_key(actor_ref: &ActorRef) -> Result<String, StreamError> {
    actor_ref.canonical_path().map(|path| path.to_canonical_uri()).ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  fn stream_error_from_context(message: impl Into<String>) -> StreamError {
    StreamError::failed_with_context(message.into())
  }

  fn set_startup_result(&mut self, result: Result<(), StreamError>) {
    if let Err(error) = result
      && self.startup_error.is_none()
    {
      self.startup_error = Some(error);
    }
  }

  fn endpoint_actor_ref(&self) -> Result<ActorRef, StreamError> {
    self
      .endpoint_actor
      .as_ref()
      .map(|endpoint_actor| endpoint_actor.actor_ref().clone())
      .ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  fn target_actor(&self) -> Result<ActorRef, StreamError> {
    self.target_actor.clone().ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  fn drain_endpoint_actor(&self) -> Result<(), StreamError> {
    match &self.endpoint_actor {
      | Some(endpoint_actor) => endpoint_actor.drain_pending(),
      | None => Ok(()),
    }
  }

  fn watch_target_actor(&self) -> Result<(), StreamError> {
    let Some(endpoint_actor) = &self.endpoint_actor else {
      return Err(StreamError::StreamRefTargetNotInitialized);
    };
    let target_actor = self.target_actor()?;
    endpoint_actor.watch(&target_actor)
  }

  fn release_target_watch(&self) -> Result<(), StreamError> {
    let Some(endpoint_actor) = &self.endpoint_actor else {
      return Ok(());
    };
    let target_actor = self.target_actor()?;
    endpoint_actor.unwatch(&target_actor)
  }

  fn send_handshake(&mut self) -> Result<(), StreamError> {
    let endpoint_actor_ref = self.endpoint_actor_ref()?;
    let target_ref_path = Self::actor_key(&endpoint_actor_ref)?;
    let message = StreamRefOnSubscribeHandshake::new(target_ref_path);
    self.send_to_target(message)
  }

  fn send_to_target<M>(&self, message: M) -> Result<(), StreamError>
  where
    M: Send + Sync + 'static, {
    let mut target_actor = self.target_actor()?;
    let endpoint_actor_ref = self.endpoint_actor_ref()?;
    target_actor
      .try_tell(AnyMessage::new(message).with_sender(endpoint_actor_ref))
      .map_err(|error| StreamError::from_send_error(&error))
  }

  fn serialize_value(&self, value: &T) -> Result<SerializedMessage, StreamError>
  where
    T: Send + Sync + 'static, {
    let Some(system) = &self.system else {
      return Err(StreamError::StreamRefTargetNotInitialized);
    };
    let extension = system.extended().register_extension(&default_serialization_extension_id());
    extension
      .with_read(|serialization| serialization.serialize(value, SerializationCallScope::Remote))
      .map_err(|error| Self::stream_error_from_context(format!("StreamRef payload serialization failed: {error:?}")))
  }

  fn maybe_request_input(&self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    self.state.error_result()?;
    if !self.state.can_accept_input() {
      return Ok(false);
    }
    if demand.has_demand() {
      return Ok(false);
    }
    demand.request(1)?;
    Ok(true)
  }
}

impl<T> SinkLogic for ActorBackedSinkRefLogic<T>
where
  T: Send + Sync + 'static,
{
  fn can_accept_input(&self) -> bool {
    self.startup_error.is_none() && self.state.can_accept_input()
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    if let Some(error) = &self.startup_error {
      return Err(error.clone());
    }
    self.drain_endpoint_actor()?;
    let _progressed = self.maybe_request_input(demand)?;
    Ok(())
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    self.drain_endpoint_actor()?;
    if let Some(error) = &self.startup_error {
      return Err(error.clone());
    }
    let value = downcast_value::<T>(input)?;
    let serialized = self.serialize_value(&value)?;
    let seq_nr = self.state.reserve_next_seq_nr()?;
    self.send_to_target(StreamRefSequencedOnNext::new(seq_nr, serialized))?;
    let _progressed = self.maybe_request_input(demand)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let seq_nr = self.state.next_seq_nr();
    self.send_to_target(StreamRefRemoteStreamCompleted::new(seq_nr))?;
    self.release_target_watch()
  }

  fn on_error(&mut self, error: StreamError) {
    if let Err(send_error) = self.send_to_target(StreamRefRemoteStreamFailure::new(format!("{error}"))) {
      self.state.fail(send_error);
    }
    if let Err(release_error) = self.release_target_watch() {
      self.state.fail(release_error);
    }
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    if let Some(error) = &self.startup_error {
      return Err(error.clone());
    }
    self.drain_endpoint_actor()?;
    self.maybe_request_input(demand)
  }

  fn attach_actor_system(&mut self, system: ActorSystem) {
    let Some(target_actor) = &self.target_actor else {
      return;
    };
    let target_actor_key = Self::actor_key(target_actor);
    let endpoint_actor =
      StageActor::new(&system, Box::new(ActorBackedSinkRefReceive::new(self.state.clone(), target_actor_key)));
    self.system = Some(system);
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

#[derive(Clone)]
struct ActorBackedSinkRefStateShared {
  inner: ArcShared<SpinSyncMutex<ActorBackedSinkRefState>>,
}

impl ActorBackedSinkRefStateShared {
  fn new() -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(ActorBackedSinkRefState::new())) }
  }

  fn subscribe(&self) {
    self.inner.lock().subscribed = true;
  }

  fn accept_demand(&self, seq_nr: u64, demand: NonZeroU64) -> Result<(), StreamError> {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    if seq_nr != guard.next_out_seq_nr {
      return Err(StreamError::InvalidSequenceNumber {
        expected_seq_nr: guard.next_out_seq_nr,
        got_seq_nr:      seq_nr,
        message:         Cow::Borrowed("invalid stream ref sequence number"),
      });
    }
    guard.pending_remote_demand = guard.pending_remote_demand.saturating_add(demand.get());
    Ok(())
  }

  fn reserve_next_seq_nr(&self) -> Result<u64, StreamError> {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    if !guard.subscribed || guard.pending_remote_demand == 0 {
      return Err(StreamError::WouldBlock);
    }
    let seq_nr = guard.next_out_seq_nr;
    guard.next_out_seq_nr = guard.next_out_seq_nr.saturating_add(1);
    guard.pending_remote_demand = guard.pending_remote_demand.saturating_sub(1);
    Ok(seq_nr)
  }

  fn next_seq_nr(&self) -> u64 {
    self.inner.lock().next_out_seq_nr
  }

  fn can_accept_input(&self) -> bool {
    let guard = self.inner.lock();
    guard.failure.is_none() && guard.subscribed && guard.pending_remote_demand > 0
  }

  fn error_result(&self) -> Result<(), StreamError> {
    match &self.inner.lock().failure {
      | Some(error) => Err(error.clone()),
      | None => Ok(()),
    }
  }

  fn fail(&self, error: StreamError) {
    let mut guard = self.inner.lock();
    if guard.failure.is_none() {
      guard.failure = Some(error);
    }
  }
}

struct ActorBackedSinkRefState {
  subscribed:            bool,
  pending_remote_demand: u64,
  next_out_seq_nr:       u64,
  failure:               Option<StreamError>,
}

impl ActorBackedSinkRefState {
  const fn new() -> Self {
    Self {
      subscribed:            false,
      pending_remote_demand: 0,
      next_out_seq_nr:       0,
      failure:               None,
    }
  }
}

struct ActorBackedSinkRefReceive {
  state:            ActorBackedSinkRefStateShared,
  target_actor_key: Result<String, StreamError>,
}

impl ActorBackedSinkRefReceive {
  const fn new(state: ActorBackedSinkRefStateShared, target_actor_key: Result<String, StreamError>) -> Self {
    Self { state, target_actor_key }
  }

  fn actor_key(actor_ref: &ActorRef) -> Result<String, StreamError> {
    actor_ref.canonical_path().map(|path| path.to_canonical_uri()).ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  fn ensure_sender(&self, sender: &ActorRef) -> Result<(), StreamError> {
    let expected_ref = self.target_actor_key.clone()?;
    let got_ref = Self::actor_key(sender)?;
    if expected_ref == got_ref {
      return Ok(());
    }
    Err(StreamError::InvalidPartnerActor {
      expected_ref: expected_ref.into(),
      got_ref:      got_ref.into(),
      message:      "stream ref message came from a non-partner actor".into(),
    })
  }

  fn accept_partner_terminated(&self, terminated: &Pid) -> Result<(), StreamError> {
    let error = StreamError::RemoteStreamRefActorTerminated {
      message: format!("remote stream ref partner actor terminated: {terminated:?}").into(),
    };
    self.state.fail(error.clone());
    Err(error)
  }
}

impl StageActorReceive for ActorBackedSinkRefReceive {
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    if let Some(SystemMessage::DeathWatchNotification(terminated)) = envelope.message().downcast_ref::<SystemMessage>()
    {
      return self.accept_partner_terminated(terminated);
    }
    self.ensure_sender(envelope.sender())?;
    if envelope.message().downcast_ref::<StreamRefAck>().is_some() {
      self.state.subscribe();
      return Ok(());
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefCumulativeDemand>() {
      return self.state.accept_demand(message.seq_nr(), message.demand());
    }
    if let Some(message) = envelope.message().downcast_ref::<StreamRefRemoteStreamFailure>() {
      self.state.fail(StreamError::failed_with_context(String::from(message.message())));
      return Ok(());
    }
    Err(StreamError::Failed)
  }
}
