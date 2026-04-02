//! Typed actor system wrapper.

#[cfg(test)]
mod tests;

use alloc::{format, string::String, vec::Vec};
use core::{marker::PhantomData, time::Duration};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  kernel::{
    actor::{
      Address, actor_ref::dead_letter::DeadLetterEntry, error::SendError, extension::ExtensionId, messaging::AskResult,
      setup::ActorSystemConfig, spawn::SpawnError,
    },
    event::{
      logging::LogLevel,
      stream::{EventStreamEvent, EventStreamShared, EventStreamSubscriberShared, EventStreamSubscription},
    },
    system::{ActorSystem, state::SystemStateShared},
    util::futures::ActorFutureShared,
  },
  typed::{
    TypedActorRef, TypedActorSystemLog, TypedActorSystemSettings,
    actor::TypedChildRef,
    dispatchers::Dispatchers,
    internal::TypedSchedulerShared,
    props::TypedProps,
    receptionist::{ReceptionistCommand, SYSTEM_RECEPTIONIST_TOP_LEVEL},
    scheduler::Scheduler,
  },
};

/// Actor system facade that enforces a message type `M` at the API boundary.
pub struct TypedActorSystem<M>
where
  M: Send + Sync + 'static, {
  inner:          ActorSystem,
  cached_address: Address,
  marker:         PhantomData<M>,
}

impl<M> TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  /// Creates an empty actor system without any guardian (testing only).
  #[must_use]
  #[cfg(any(test, feature = "test-support"))]
  pub fn new_empty() -> Self {
    let inner = ActorSystem::new_empty();
    let cached_address = Address::local(inner.name());
    Self { inner, cached_address, marker: PhantomData }
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
    guardian: &TypedProps<M>,
    tick_driver_config: crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig,
  ) -> Result<Self, SpawnError> {
    let inner = ActorSystem::new(guardian.to_untyped(), tick_driver_config)?;
    let cached_address = Address::local(inner.name());
    Ok(Self { inner, cached_address, marker: PhantomData })
  }

  /// Creates a typed actor system using the supplied configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if guardian initialization fails.
  pub fn new_with_config(guardian: &TypedProps<M>, config: &ActorSystemConfig) -> Result<Self, SpawnError> {
    let inner = ActorSystem::new_with_config(guardian.to_untyped(), config)?;
    let cached_address = Address::local(inner.name());
    Ok(Self { inner, cached_address, marker: PhantomData })
  }
}

impl<M> TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  /// Returns the typed user guardian reference.
  #[must_use]
  pub fn user_guardian_ref(&self) -> TypedActorRef<M> {
    TypedActorRef::from_untyped(self.inner.user_guardian_ref())
  }

  /// Returns the untyped system for advanced scenarios.
  #[must_use]
  pub const fn as_untyped(&self) -> &ActorSystem {
    &self.inner
  }

  /// Consumes the typed wrapper and returns the untyped system.
  #[must_use]
  pub fn into_untyped(self) -> ActorSystem {
    self.inner
  }

  /// Returns the shared system state handle.
  #[must_use]
  pub fn state(&self) -> SystemStateShared {
    self.inner.state()
  }

  /// Returns the system receptionist reference when available.
  #[must_use]
  pub fn receptionist_ref(&self) -> Option<TypedActorRef<ReceptionistCommand>> {
    self.inner.state().extra_top_level(SYSTEM_RECEPTIONIST_TOP_LEVEL).map(TypedActorRef::from_untyped)
  }

  /// Allocates a new pid (testing helper).
  #[must_use]
  pub fn allocate_pid(&self) -> crate::core::kernel::actor::Pid {
    self.inner.allocate_pid()
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamShared {
    self.inner.event_stream()
  }

  /// Subscribes the provided observer to the event stream.
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &EventStreamSubscriberShared) -> EventStreamSubscription {
    self.inner.subscribe_event_stream(subscriber)
  }

  /// Returns a snapshot of recorded dead letters.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntry> {
    self.inner.dead_letters()
  }

  /// Emits a log event with the specified severity.
  pub fn emit_log(
    &self,
    level: LogLevel,
    message: impl Into<String>,
    origin: Option<crate::core::kernel::actor::Pid>,
    logger_name: Option<String>,
  ) {
    self.inner.emit_log(level, message, origin, logger_name)
  }

  /// Returns the configured actor system name.
  ///
  /// Corresponds to Pekko's `ActorSystem.name`.
  #[must_use]
  pub fn name(&self) -> String {
    self.inner.name()
  }

  /// Returns the default address of this actor system.
  ///
  /// Corresponds to Pekko's `ActorSystem.address`.
  #[must_use]
  pub fn address(&self) -> Address {
    self.cached_address.clone()
  }

  /// Returns the start time of the actor system (epoch-relative duration).
  ///
  /// Corresponds to Pekko's `ActorSystem.startTime`.
  #[must_use]
  pub const fn start_time(&self) -> Duration {
    self.inner.start_time()
  }

  /// Returns the elapsed time since the system was started.
  ///
  /// In `no_std` environments the caller must provide the current time.
  /// Corresponds to Pekko's `ActorSystem.uptime`.
  #[must_use]
  pub const fn uptime(&self, now: Duration) -> Duration {
    now.saturating_sub(self.start_time())
  }

  /// Returns the immutable settings snapshot preserved by the underlying actor system.
  ///
  /// Corresponds to Pekko's `ActorSystem.settings`.
  #[must_use]
  pub fn settings(&self) -> TypedActorSystemSettings {
    self.inner.settings()
  }

  /// Emits a summary of the current system configuration to the event stream.
  ///
  /// Corresponds to Pekko's `ActorSystem.logConfiguration()`.
  pub fn log_configuration(&self) {
    let settings = self.settings();
    self.log().emit(
      LogLevel::Info,
      format!(
        "typed actor system configuration: system_name={}, start_time={:?}",
        settings.system_name(),
        settings.start_time()
      ),
    );
  }

  /// Returns the system-level log handle.
  ///
  /// Corresponds to Pekko's `ActorSystem.log`.
  #[must_use]
  pub fn log(&self) -> TypedActorSystemLog {
    TypedActorSystemLog::new(self.inner.clone())
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent) {
    self.inner.publish_event(event)
  }

  /// Spawns a new top-level actor under the user guardian.
  #[allow(dead_code)]
  pub(crate) fn spawn<C>(&self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner.spawn(typed_props.to_untyped())?;
    Ok(TypedChildRef::from_untyped(child))
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn when_terminated(&self) -> ActorFutureShared<()> {
    self.inner.when_terminated()
  }

  /// Returns a future that resolves once the actor system terminates.
  ///
  /// Corresponds to Pekko's Java API alias `ActorSystem.getWhenTerminated`.
  #[must_use]
  pub fn get_when_terminated(&self) -> ActorFutureShared<()> {
    self.when_terminated()
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if the terminate signal cannot be sent.
  pub fn terminate(&self) -> Result<(), SendError> {
    self.inner.terminate()
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ActorFutureShared<AskResult>> {
    self.inner.drain_ready_ask_futures()
  }

  /// Wraps an existing untyped actor system so typed APIs can mirror its services.
  #[must_use]
  pub fn from_untyped(system: ActorSystem) -> Self {
    let cached_address = Address::local(system.name());
    Self { inner: system, cached_address, marker: PhantomData }
  }

  /// Returns the typed scheduler facade.
  ///
  /// Corresponds to Pekko's `ActorSystem.scheduler`.
  #[must_use]
  pub fn scheduler(&self) -> Scheduler {
    Scheduler::new(TypedSchedulerShared::new(self.inner.scheduler()))
  }

  /// Returns the raw typed scheduler shared handle for internal wiring.
  #[must_use]
  pub(crate) fn raw_scheduler(&self) -> TypedSchedulerShared {
    TypedSchedulerShared::new(self.inner.scheduler())
  }

  /// Returns the typed dispatcher lookup facade.
  ///
  /// Corresponds to Pekko's `ActorSystem.dispatchers`.
  #[must_use]
  pub fn dispatchers(&self) -> Dispatchers {
    Dispatchers::new(self.inner.state())
  }

  /// Returns a delay provider backed by the scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> crate::core::kernel::actor::scheduler::SchedulerBackedDelayProvider {
    self.inner.delay_provider()
  }

  /// Returns the extension registered for the given identifier.
  ///
  /// Corresponds to Pekko's `ActorSystem.extension`.
  #[must_use]
  pub fn extension<E>(&self, ext_id: &E) -> Option<ArcShared<E::Ext>>
  where
    E: ExtensionId, {
    self.inner.extended().extension(ext_id)
  }

  /// Returns whether an extension has been registered.
  ///
  /// Corresponds to Pekko's `ActorSystem.hasExtension`.
  #[must_use]
  pub fn has_extension<E>(&self, ext_id: &E) -> bool
  where
    E: ExtensionId, {
    self.inner.extended().has_extension(ext_id)
  }

  /// Registers an extension if not already present (putIfAbsent semantics).
  ///
  /// Corresponds to Pekko's `ActorSystem.registerExtension`.
  pub fn register_extension<E>(&self, ext_id: &E) -> ArcShared<E::Ext>
  where
    E: ExtensionId, {
    self.inner.extended().register_extension(ext_id)
  }
}

impl<M> Clone for TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self {
      inner:          self.inner.clone(),
      cached_address: self.cached_address.clone(),
      marker:         PhantomData,
    }
  }
}
