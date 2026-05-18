//! Receptionist runtime implementation details.

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};
use core::any::TypeId;

use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::ActorRef,
    error::ActorError,
    extension::{Extension, ExtensionId},
    props::Props,
    spawn::SpawnError,
  },
  event::logging::LogLevel,
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock, shared::Shared};

use super::{Deregistered, Listing, ReceptionistCommand, Registered, ServiceKey};
use crate::{
  TypedActorRef, TypedActorSystem, TypedProps, behavior::Behavior, dsl::Behaviors, message_and_signals::BehaviorSignal,
};

/// Composite key for internal registry lookups.
type RegistryKey = (String, TypeId);

/// Name used for the system-level receptionist top-level registration.
pub const SYSTEM_RECEPTIONIST_TOP_LEVEL: &str = "receptionist";

/// Internal state for the receptionist actor.
pub(crate) struct ReceptionistState {
  pub(crate) registrations: BTreeMap<RegistryKey, Vec<ActorRef>>,
  pub(crate) subscribers:   BTreeMap<RegistryKey, Vec<TypedActorRef<Listing>>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ReceptionistExtensionId;

pub(crate) enum WatchTarget {
  RegisteredActor(ActorRef),
  Subscriber(TypedActorRef<Listing>),
}

struct ReceptionistCommandContext<'a, Watch> {
  state:        &'a mut ReceptionistState,
  system:       &'a ActorSystem,
  origin:       Option<Pid>,
  watch_target: &'a mut Watch,
}

/// Receptionist actor that manages service registrations and subscriptions.
///
/// Use [`Receptionist::behavior`] to obtain the initial behavior for the receptionist actor.
/// Interact with it by sending [`ReceptionistCommand`] messages via its `TypedActorRef`.
#[derive(Clone)]
pub struct Receptionist {
  actor_ref: TypedActorRef<ReceptionistCommand>,
}

impl ReceptionistExtensionId {
  pub(crate) const fn new() -> Self {
    Self
  }
}

impl Extension for Receptionist {}

impl ExtensionId for ReceptionistExtensionId {
  type Ext = Receptionist;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    Receptionist::from_actor_ref(Receptionist::resolve_actor_ref(system))
  }
}

impl Receptionist {
  fn empty_state() -> SharedLock<ReceptionistState> {
    SharedLock::new_with_driver::<DefaultMutex<_>>(ReceptionistState {
      registrations: BTreeMap::new(),
      subscribers:   BTreeMap::new(),
    })
  }

  const fn from_actor_ref(actor_ref: TypedActorRef<ReceptionistCommand>) -> Self {
    Self { actor_ref }
  }

  fn extension_props() -> Props {
    TypedProps::<ReceptionistCommand>::from_behavior_factory(Self::behavior)
      .to_untyped()
      .clone()
      .with_name("receptionist-extension")
  }

  fn spawn_extension_actor(system: &ActorSystem) -> Result<TypedActorRef<ReceptionistCommand>, SpawnError> {
    let child = system.extended().spawn_system_actor(&Self::extension_props())?;
    Ok(TypedActorRef::from_untyped(child.into_actor_ref()))
  }

  fn resolve_actor_ref(system: &ActorSystem) -> TypedActorRef<ReceptionistCommand> {
    if let Some(actor_ref) = system.state().extra_top_level(SYSTEM_RECEPTIONIST_TOP_LEVEL) {
      return TypedActorRef::from_untyped(actor_ref);
    }

    Self::spawn_extension_actor(system).unwrap_or_else(|error| {
      panic!("receptionist extension actor must be spawnable in empty systems: {error:?}");
    })
  }

  fn ensure_extension<M>(system: &TypedActorSystem<M>) -> ArcShared<Self>
  where
    M: Send + Sync + 'static, {
    system.register_extension(&ReceptionistExtensionId::new())
  }

  /// Returns the receptionist extension for the provided system.
  #[must_use]
  pub fn get<M>(system: &TypedActorSystem<M>) -> Self
  where
    M: Send + Sync + 'static, {
    let registered = Self::ensure_extension(system);
    registered.with_ref(|receptionist: &Receptionist| receptionist.clone())
  }

  /// Creates the receptionist extension for the provided system.
  #[must_use]
  pub fn create_extension<M>(system: &TypedActorSystem<M>) -> Self
  where
    M: Send + Sync + 'static, {
    Self::ensure_extension(system).with_ref(|receptionist: &Receptionist| receptionist.clone())
  }

  /// Returns the receptionist actor reference held by this facade.
  #[must_use]
  pub fn r#ref(&self) -> TypedActorRef<ReceptionistCommand> {
    self.actor_ref.clone()
  }

  /// Returns the initial behavior for the Receptionist actor.
  #[must_use]
  pub fn behavior() -> Behavior<ReceptionistCommand> {
    let state = Self::empty_state();
    let state_for_message = state.clone();
    let state_for_signal = state;

    Behaviors::receive_message(move |ctx, cmd| {
      let typed_system = ctx.system();
      let system = typed_system.as_untyped();
      state_for_message.with_lock(|guard| {
        handle_command(guard, system, Some(ctx.pid()), cmd, |watch_target| match watch_target {
          | WatchTarget::RegisteredActor(actor_ref) => ctx
            .as_untyped_mut()
            .watch(&actor_ref)
            .map_err(|error| ActorError::recoverable(alloc::format!("watch failed: {:?}", error))),
          | WatchTarget::Subscriber(subscriber) => {
            ctx.watch(&subscriber).map_err(|error| ActorError::recoverable(alloc::format!("watch failed: {:?}", error)))
          },
        })
      })?;
      Ok(Behaviors::same())
    })
    .receive_signal(move |ctx, signal| {
      let BehaviorSignal::Terminated(terminated) = signal else {
        return Ok(Behaviors::same());
      };
      let terminated_pid = terminated.pid();

      state_for_signal.with_lock(|guard| {
        let mut updated_keys = Vec::new();
        for (key, refs) in &mut guard.registrations {
          let before = refs.len();
          refs.retain(|entry| entry.pid() != terminated_pid);
          if refs.len() != before {
            updated_keys.push(key.clone());
          }
        }
        guard.registrations.retain(|_, refs| !refs.is_empty());

        for subscribers in guard.subscribers.values_mut() {
          subscribers.retain(|subscriber| subscriber.pid() != terminated_pid);
        }
        guard.subscribers.retain(|_, subscribers| !subscribers.is_empty());

        for key in &updated_keys {
          notify_subscribers(&guard.subscribers, key, &guard.registrations, ctx.system().as_untyped(), None);
        }
      });
      Ok(Behaviors::same())
    })
  }

  /// Creates a [`Register`](ReceptionistCommand::Register) command from a typed service key.
  #[must_use]
  pub fn register<M>(key: &ServiceKey<M>, actor_ref: TypedActorRef<M>) -> ReceptionistCommand
  where
    M: Send + Sync + 'static, {
    ReceptionistCommand::Register {
      service_id: key.id().into(),
      type_id:    key.type_id(),
      actor_ref:  actor_ref.into_untyped(),
      reply_to:   None,
    }
  }

  /// Creates a [`Register`](ReceptionistCommand::Register) command with an acknowledgement target.
  ///
  /// Corresponds to Pekko's `Receptionist.Register` with a `replyTo` parameter.
  #[must_use]
  pub fn register_with_ack<M>(
    key: &ServiceKey<M>,
    actor_ref: TypedActorRef<M>,
    reply_to: TypedActorRef<Registered>,
  ) -> ReceptionistCommand
  where
    M: Send + Sync + 'static, {
    ReceptionistCommand::Register {
      service_id: key.id().into(),
      type_id:    key.type_id(),
      actor_ref:  actor_ref.into_untyped(),
      reply_to:   Some(reply_to),
    }
  }

  /// Creates a [`Deregister`](ReceptionistCommand::Deregister) command from a typed service key.
  #[must_use]
  pub fn deregister<M>(key: &ServiceKey<M>, actor_ref: TypedActorRef<M>) -> ReceptionistCommand
  where
    M: Send + Sync + 'static, {
    ReceptionistCommand::Deregister {
      service_id: key.id().into(),
      type_id:    key.type_id(),
      actor_ref:  actor_ref.into_untyped(),
      reply_to:   None,
    }
  }

  /// Creates a [`Deregister`](ReceptionistCommand::Deregister) command with an acknowledgement
  /// target.
  ///
  /// Corresponds to Pekko's `Receptionist.Deregister` with a `replyTo` parameter.
  #[must_use]
  pub fn deregister_with_ack<M>(
    key: &ServiceKey<M>,
    actor_ref: TypedActorRef<M>,
    reply_to: TypedActorRef<Deregistered>,
  ) -> ReceptionistCommand
  where
    M: Send + Sync + 'static, {
    ReceptionistCommand::Deregister {
      service_id: key.id().into(),
      type_id:    key.type_id(),
      actor_ref:  actor_ref.into_untyped(),
      reply_to:   Some(reply_to),
    }
  }

  /// Creates a [`Subscribe`](ReceptionistCommand::Subscribe) command from a typed service key.
  #[must_use]
  pub fn subscribe<M>(key: &ServiceKey<M>, subscriber: TypedActorRef<Listing>) -> ReceptionistCommand
  where
    M: Send + Sync + 'static, {
    ReceptionistCommand::Subscribe { service_id: key.id().into(), type_id: key.type_id(), subscriber }
  }

  /// Creates an [`Unsubscribe`](ReceptionistCommand::Unsubscribe) command from a typed service key.
  #[must_use]
  pub fn unsubscribe<M>(key: &ServiceKey<M>, subscriber: TypedActorRef<Listing>) -> ReceptionistCommand
  where
    M: Send + Sync + 'static, {
    ReceptionistCommand::Unsubscribe { service_id: key.id().into(), type_id: key.type_id(), subscriber }
  }

  /// Creates a [`Find`](ReceptionistCommand::Find) command from a typed service key.
  #[must_use]
  pub fn find<M>(key: &ServiceKey<M>, reply_to: TypedActorRef<Listing>) -> ReceptionistCommand
  where
    M: Send + Sync + 'static, {
    ReceptionistCommand::Find { service_id: key.id().into(), type_id: key.type_id(), reply_to }
  }
}

pub(crate) fn handle_command<Watch>(
  state: &mut ReceptionistState,
  system: &ActorSystem,
  origin: Option<Pid>,
  command: &ReceptionistCommand,
  mut watch_target: Watch,
) -> Result<(), ActorError>
where
  Watch: FnMut(WatchTarget) -> Result<(), ActorError>, {
  let mut context = ReceptionistCommandContext { state, system, origin, watch_target: &mut watch_target };
  match command {
    | ReceptionistCommand::Register { service_id, type_id, actor_ref, reply_to } => {
      handle_register_command(&mut context, service_id, *type_id, actor_ref, reply_to)?;
    },
    | ReceptionistCommand::Deregister { service_id, type_id, actor_ref, reply_to } => {
      handle_deregister_command(&mut context, service_id, *type_id, actor_ref, reply_to);
    },
    | ReceptionistCommand::Subscribe { service_id, type_id, subscriber } => {
      handle_subscribe_command(&mut context, service_id, *type_id, subscriber)?;
    },
    | ReceptionistCommand::Unsubscribe { service_id, type_id, subscriber } => {
      handle_unsubscribe_command(&mut context, service_id, *type_id, subscriber);
    },
    | ReceptionistCommand::Find { service_id, type_id, reply_to } => {
      handle_find_command(&mut context, service_id, *type_id, reply_to);
    },
  }

  Ok(())
}

fn handle_register_command<Watch>(
  context: &mut ReceptionistCommandContext<'_, Watch>,
  service_id: &str,
  type_id: TypeId,
  actor_ref: &ActorRef,
  reply_to: &Option<TypedActorRef<Registered>>,
) -> Result<(), ActorError>
where
  Watch: FnMut(WatchTarget) -> Result<(), ActorError>, {
  let key = (service_id.to_string(), type_id);
  let already_registered = context
    .state
    .registrations
    .get(&key)
    .is_some_and(|entry| entry.iter().any(|existing| existing.pid() == actor_ref.pid()));
  if !already_registered {
    (context.watch_target)(WatchTarget::RegisteredActor(actor_ref.clone()))?;
    context.state.registrations.entry(key.clone()).or_default().push(actor_ref.clone());
    notify_subscribers(&context.state.subscribers, &key, &context.state.registrations, context.system, context.origin);
  }
  if let Some(reply_to) = reply_to.clone() {
    let ack = Registered::new(service_id.to_string(), type_id, actor_ref.clone());
    try_send_registered_ack(reply_to, ack, context.system, context.origin);
  }
  Ok(())
}

fn handle_deregister_command<Watch>(
  context: &mut ReceptionistCommandContext<'_, Watch>,
  service_id: &str,
  type_id: TypeId,
  actor_ref: &ActorRef,
  reply_to: &Option<TypedActorRef<Deregistered>>,
) {
  let key = (service_id.to_string(), type_id);
  let registrations_updated = remove_receptionist_registration(context.state, &key, actor_ref);
  if registrations_updated {
    notify_subscribers(&context.state.subscribers, &key, &context.state.registrations, context.system, context.origin);
  }
  if let Some(reply_to) = reply_to.clone() {
    let ack = Deregistered::new(service_id.to_string(), type_id, actor_ref.clone());
    try_send_deregistered_ack(reply_to, ack, context.system, context.origin);
  }
}

fn remove_receptionist_registration(state: &mut ReceptionistState, key: &RegistryKey, actor_ref: &ActorRef) -> bool {
  let mut registrations_updated = false;
  let mut remove_key = false;
  if let Some(entry) = state.registrations.get_mut(key) {
    let before = entry.len();
    entry.retain(|existing| existing.pid() != actor_ref.pid());
    registrations_updated = entry.len() != before;
    remove_key = entry.is_empty();
  }
  if remove_key {
    state.registrations.remove(key);
  }
  registrations_updated
}

fn handle_subscribe_command<Watch>(
  context: &mut ReceptionistCommandContext<'_, Watch>,
  service_id: &str,
  type_id: TypeId,
  subscriber: &TypedActorRef<Listing>,
) -> Result<(), ActorError>
where
  Watch: FnMut(WatchTarget) -> Result<(), ActorError>, {
  let key = (service_id.to_string(), type_id);
  send_initial_listing(context, service_id, type_id, subscriber, &key);
  let already_subscribed = context
    .state
    .subscribers
    .get(&key)
    .is_some_and(|subscribers| subscribers.iter().any(|existing| existing.pid() == subscriber.pid()));
  if !already_subscribed {
    (context.watch_target)(WatchTarget::Subscriber(subscriber.clone()))?;
    context.state.subscribers.entry(key).or_default().push(subscriber.clone());
  }
  Ok(())
}

fn send_initial_listing<Watch>(
  context: &ReceptionistCommandContext<'_, Watch>,
  service_id: &str,
  type_id: TypeId,
  subscriber: &TypedActorRef<Listing>,
  key: &RegistryKey,
) {
  let current = context.state.registrations.get(key).cloned().unwrap_or_default();
  let listing = Listing::new(service_id.to_string(), type_id, current);
  let mut typed_subscriber = subscriber.clone();
  if let Err(error) = typed_subscriber.try_tell(listing) {
    context.system.emit_log(
      LogLevel::Warn,
      alloc::format!(
        "receptionist failed to send initial listing to subscriber {:?} for service_id={} type_id={:?}: {:?}",
        subscriber.pid(),
        service_id,
        type_id,
        error
      ),
      context.origin,
      None,
    );
  }
}

fn handle_unsubscribe_command<Watch>(
  context: &mut ReceptionistCommandContext<'_, Watch>,
  service_id: &str,
  type_id: TypeId,
  subscriber: &TypedActorRef<Listing>,
) {
  let key = (service_id.to_string(), type_id);
  let mut remove_key = false;
  if let Some(subscribers) = context.state.subscribers.get_mut(&key) {
    subscribers.retain(|existing| existing.pid() != subscriber.pid());
    remove_key = subscribers.is_empty();
  }
  if remove_key {
    context.state.subscribers.remove(&key);
  }
}

fn handle_find_command<Watch>(
  context: &mut ReceptionistCommandContext<'_, Watch>,
  service_id: &str,
  type_id: TypeId,
  reply_to: &TypedActorRef<Listing>,
) {
  let key = (service_id.to_string(), type_id);
  let current = context.state.registrations.get(&key).cloned().unwrap_or_default();
  let listing = Listing::new(service_id.to_string(), type_id, current);
  let mut reply_to = reply_to.clone();
  if let Err(error) = reply_to.try_tell(listing) {
    context.system.emit_log(
      LogLevel::Warn,
      alloc::format!(
        "receptionist failed to reply with listing to {:?} for service_id={} type_id={:?}: {:?}",
        reply_to.pid(),
        service_id,
        type_id,
        error
      ),
      context.origin,
      None,
    );
  }
}

fn try_send_registered_ack(
  mut reply_to: TypedActorRef<Registered>,
  ack: Registered,
  system: &ActorSystem,
  origin: Option<Pid>,
) {
  // ACK delivery is best-effort: the registration itself already succeeded,
  // so a failed ack does not invalidate the operation.
  if let Err(error) = reply_to.try_tell(ack) {
    system.emit_log(
      LogLevel::Warn,
      alloc::format!("receptionist failed to send Registered ack: {:?}", error),
      origin,
      None,
    );
  }
}

fn try_send_deregistered_ack(
  mut reply_to: TypedActorRef<Deregistered>,
  ack: Deregistered,
  system: &ActorSystem,
  origin: Option<Pid>,
) {
  // ACK delivery is best-effort: the deregistration itself already succeeded,
  // so a failed ack does not invalidate the operation.
  if let Err(error) = reply_to.try_tell(ack) {
    system.emit_log(
      LogLevel::Warn,
      alloc::format!("receptionist failed to send Deregistered ack: {:?}", error),
      origin,
      None,
    );
  }
}

/// Notifies all subscribers of a key about the current registration set.
fn notify_subscribers(
  subscribers: &BTreeMap<RegistryKey, Vec<TypedActorRef<Listing>>>,
  key: &RegistryKey,
  registrations: &BTreeMap<RegistryKey, Vec<ActorRef>>,
  system: &ActorSystem,
  origin: Option<Pid>,
) {
  if let Some(subs) = subscribers.get(key) {
    let refs = registrations.get(key).cloned().unwrap_or_default();
    let listing = Listing::new(key.0.clone(), key.1, refs);
    for sub in subs {
      let mut s = sub.clone();
      if let Err(error) = s.try_tell(listing.clone()) {
        system.emit_log(
          LogLevel::Warn,
          alloc::format!(
            "receptionist failed to notify subscriber {:?} for service_id={} type_id={:?}: {:?}",
            sub.pid(),
            key.0,
            key.1,
            error
          ),
          origin,
          None,
        );
      }
    }
  }
}
