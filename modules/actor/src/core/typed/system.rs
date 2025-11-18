//! Typed actor system wrapper.

use alloc::{string::String, vec::Vec};
use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::core::{
  config::ActorSystemConfig,
  dead_letter::DeadLetterEntryGeneric,
  error::SendError,
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric},
  futures::ActorFuture,
  logging::LogLevel,
  messaging::AnyMessageGeneric,
  spawn::SpawnError,
  system::{ActorSystemGeneric, SystemStateGeneric},
  typed::{
    actor_prim::{TypedActorRefGeneric, TypedChildRefGeneric},
    props::TypedPropsGeneric,
    scheduler::TypedSchedulerContext,
  },
};

/// Actor system facade that enforces a message type `M` at the API boundary.
pub struct TypedActorSystemGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  inner:  ActorSystemGeneric<TB>,
  marker: PhantomData<M>,
}

/// Type alias for [TypedActorSystemGeneric] with the default [NoStdToolbox].
pub type TypedActorSystem<M> = TypedActorSystemGeneric<M, NoStdToolbox>;

impl<M, TB> TypedActorSystemGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + Default + 'static,
{
  /// Creates an empty actor system without any guardian (testing only).
  #[must_use]
  pub fn new_empty() -> Self {
    Self { inner: ActorSystemGeneric::new_empty(), marker: PhantomData }
  }

  /// Creates a new typed actor system with the required tick driver configuration.
  ///
  /// # Arguments
  ///
  /// * `guardian` - Typed properties for the user guardian actor
  /// * `tick_driver_config` - Tick driver configuration (required)
  ///
  /// # Errors
  ///
  /// Returns an error if the guardian actor cannot be spawned or tick driver setup fails.
  pub fn new(
    guardian: &TypedPropsGeneric<M, TB>,
    tick_driver_config: crate::core::scheduler::TickDriverConfig<TB>,
  ) -> Result<Self, SpawnError> {
    Ok(Self { inner: ActorSystemGeneric::new(guardian.to_untyped(), tick_driver_config)?, marker: PhantomData })
  }

  /// Creates a typed actor system using the supplied configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if guardian initialization fails.
  pub fn new_with_config(
    guardian: &TypedPropsGeneric<M, TB>,
    config: &ActorSystemConfig<TB>,
  ) -> Result<Self, SpawnError> {
    Ok(Self { inner: ActorSystemGeneric::new_with_config(guardian.to_untyped(), config)?, marker: PhantomData })
  }
}

impl<M, TB> TypedActorSystemGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Returns the typed user guardian reference.
  #[must_use]
  pub fn user_guardian_ref(&self) -> TypedActorRefGeneric<M, TB> {
    TypedActorRefGeneric::from_untyped(self.inner.user_guardian_ref())
  }

  /// Returns the untyped system for advanced scenarios.
  #[must_use]
  pub const fn as_untyped(&self) -> &ActorSystemGeneric<TB> {
    &self.inner
  }

  /// Consumes the typed wrapper and returns the untyped system.
  #[must_use]
  pub fn into_untyped(self) -> ActorSystemGeneric<TB> {
    self.inner
  }

  /// Returns the shared system state handle.
  #[must_use]
  pub fn state(&self) -> ArcShared<SystemStateGeneric<TB>> {
    self.inner.state()
  }

  /// Allocates a new pid (testing helper).
  #[must_use]
  pub fn allocate_pid(&self) -> crate::core::actor_prim::Pid {
    self.inner.allocate_pid()
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.inner.event_stream()
  }

  /// Subscribes the provided observer to the event stream.
  #[must_use]
  pub fn subscribe_event_stream(
    &self,
    subscriber: &ArcShared<dyn EventStreamSubscriber<TB>>,
  ) -> EventStreamSubscriptionGeneric<TB> {
    self.inner.subscribe_event_stream(subscriber)
  }

  /// Returns a snapshot of recorded dead letters.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntryGeneric<TB>> {
    self.inner.dead_letters()
  }

  /// Emits a log event with the specified severity.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<crate::core::actor_prim::Pid>) {
    self.inner.emit_log(level, message, origin)
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent<TB>) {
    self.inner.publish_event(event)
  }

  /// Spawns a new top-level actor under the user guardian.
  #[allow(dead_code)]
  pub(crate) fn spawn<C>(
    &self,
    typed_props: &TypedPropsGeneric<C, TB>,
  ) -> Result<TypedChildRefGeneric<C, TB>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner.spawn(typed_props.to_untyped())?;
    Ok(TypedChildRefGeneric::from_untyped(child))
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn when_terminated(&self) -> ArcShared<ActorFuture<(), TB>> {
    self.inner.when_terminated()
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if the terminate signal cannot be sent.
  pub fn terminate(&self) -> Result<(), SendError<TB>> {
    self.inner.terminate()
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>> {
    self.inner.drain_ready_ask_futures()
  }

  /// Wraps an existing untyped actor system so typed APIs can mirror its services.
  #[must_use]
  pub const fn from_untyped(system: ActorSystemGeneric<TB>) -> Self {
    Self { inner: system, marker: PhantomData }
  }

  /// Returns the typed scheduler context when the runtime has an installed scheduler service.
  #[must_use]
  pub fn scheduler_context(&self) -> Option<TypedSchedulerContext<TB>> {
    self.inner.scheduler_context().map(TypedSchedulerContext::from_shared)
  }
}

impl<M, TB> Clone for TypedActorSystemGeneric<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), marker: PhantomData }
  }
}
