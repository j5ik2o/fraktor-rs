//! Receptionist actor providing service discovery within an actor system.

mod deregistered;
mod listing;
mod receptionist_command;
mod registered;
mod service_key;
#[cfg(test)]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::any::TypeId;

pub use deregistered::Deregistered;
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, shared::Shared};
pub use listing::Listing;
pub use receptionist_command::ReceptionistCommand;
pub use registered::Registered;
pub use service_key::ServiceKey;

use crate::core::{
  kernel::{
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
  },
  typed::{TypedActorRef, TypedProps, behavior::Behavior, dsl::Behaviors, message_and_signals::BehaviorSignal},
};

/// Composite key for internal registry lookups.
type RegistryKey = (String, TypeId);

/// Name used for the system-level receptionist top-level registration.
pub const SYSTEM_RECEPTIONIST_TOP_LEVEL: &str = "receptionist";

/// Internal state for the receptionist actor.
struct ReceptionistState {
  registrations: BTreeMap<RegistryKey, Vec<ActorRef>>,
  subscribers:   BTreeMap<RegistryKey, Vec<TypedActorRef<Listing>>>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ReceptionistExtensionId;

enum WatchTarget {
  RegisteredActor(ActorRef),
  Subscriber(TypedActorRef<Listing>),
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
  const fn new() -> Self {
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
  fn empty_state() -> ArcShared<RuntimeMutex<ReceptionistState>> {
    ArcShared::new(RuntimeMutex::new(ReceptionistState {
      registrations: BTreeMap::new(),
      subscribers:   BTreeMap::new(),
    }))
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
    let child = system.spawn_detached(&Self::extension_props())?;
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

  fn register_extension_facade<M>(system: &crate::core::typed::TypedActorSystem<M>) -> ArcShared<Self>
  where
    M: Send + Sync + 'static, {
    system.register_extension(&ReceptionistExtensionId::new())
  }

  /// Returns the receptionist extension facade for the provided system.
  #[must_use]
  pub fn get<M>(system: &crate::core::typed::TypedActorSystem<M>) -> Self
  where
    M: Send + Sync + 'static, {
    let registered = Self::register_extension_facade(system);
    let extension_id = ReceptionistExtensionId::new();
    if let Some(existing) = system.extension(&extension_id) {
      debug_assert!(ArcShared::ptr_eq(&registered, &existing));
    }
    registered.with_ref(|receptionist: &Receptionist| receptionist.clone())
  }

  /// Creates the receptionist extension facade for the provided system.
  #[must_use]
  pub fn create_extension<M>(system: &crate::core::typed::TypedActorSystem<M>) -> Self
  where
    M: Send + Sync + 'static, {
    Self::register_extension_facade(system).with_ref(|receptionist: &Receptionist| receptionist.clone())
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
      let mut guard = state_for_message.lock();
      handle_command(&mut guard, system, Some(ctx.pid()), cmd, |watch_target| match watch_target {
        | WatchTarget::RegisteredActor(actor_ref) => ctx
          .as_untyped_mut()
          .watch(&actor_ref)
          .map_err(|error| ActorError::recoverable(alloc::format!("watch failed: {:?}", error))),
        | WatchTarget::Subscriber(subscriber) => {
          ctx.watch(&subscriber).map_err(|error| ActorError::recoverable(alloc::format!("watch failed: {:?}", error)))
        },
      })?;
      Ok(Behaviors::same())
    })
    .receive_signal(move |ctx, signal| {
      let BehaviorSignal::Terminated(terminated_pid) = signal else {
        return Ok(Behaviors::same());
      };

      let mut guard = state_for_signal.lock();
      let mut updated_keys = Vec::new();
      for (key, refs) in &mut guard.registrations {
        let before = refs.len();
        refs.retain(|entry| entry.pid() != *terminated_pid);
        if refs.len() != before {
          updated_keys.push(key.clone());
        }
      }
      guard.registrations.retain(|_, refs| !refs.is_empty());

      for subscribers in guard.subscribers.values_mut() {
        subscribers.retain(|subscriber| subscriber.pid() != *terminated_pid);
      }
      guard.subscribers.retain(|_, subscribers| !subscribers.is_empty());

      for key in &updated_keys {
        notify_subscribers(&guard.subscribers, key, &guard.registrations, ctx.system().as_untyped());
      }
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

fn handle_command<Watch>(
  state: &mut ReceptionistState,
  system: &ActorSystem,
  origin: Option<Pid>,
  command: &ReceptionistCommand,
  mut watch_target: Watch,
) -> Result<(), ActorError>
where
  Watch: FnMut(WatchTarget) -> Result<(), ActorError>, {
  match command {
    | ReceptionistCommand::Register { service_id, type_id, actor_ref, reply_to } => {
      let key = (service_id.clone(), *type_id);
      let entry = state.registrations.entry(key.clone()).or_default();
      if !entry.iter().any(|existing| existing.pid() == actor_ref.pid()) {
        entry.push(actor_ref.clone());
        if let Err(error) = watch_target(WatchTarget::RegisteredActor(actor_ref.clone())) {
          system.emit_log(
            LogLevel::Warn,
            alloc::format!("receptionist failed to watch registered actor: {:?}", error),
            origin,
            None,
          );
        }
        notify_subscribers(&state.subscribers, &key, &state.registrations, system);
      }
      if let Some(reply_to) = reply_to.clone() {
        let ack = Registered::new(service_id.clone(), *type_id, actor_ref.clone());
        try_send_registered_ack(reply_to, ack, system, origin);
      }
    },
    | ReceptionistCommand::Deregister { service_id, type_id, actor_ref, reply_to } => {
      let key = (service_id.clone(), *type_id);
      if let Some(entry) = state.registrations.get_mut(&key) {
        let before = entry.len();
        entry.retain(|existing| existing.pid() != actor_ref.pid());
        if entry.len() != before {
          notify_subscribers(&state.subscribers, &key, &state.registrations, system);
        }
      }
      if let Some(reply_to) = reply_to.clone() {
        let ack = Deregistered::new(service_id.clone(), *type_id, actor_ref.clone());
        try_send_deregistered_ack(reply_to, ack, system, origin);
      }
    },
    | ReceptionistCommand::Subscribe { service_id, type_id, subscriber } => {
      let key = (service_id.clone(), *type_id);
      let current = state.registrations.get(&key).cloned().unwrap_or_default();
      let listing = Listing::new(service_id.clone(), *type_id, current);
      let mut typed_subscriber = subscriber.clone();
      typed_subscriber.try_tell(listing).map_err(|error| ActorError::from_send_error(&error))?;
      let subscribers = state.subscribers.entry(key).or_default();
      if !subscribers.iter().any(|existing| existing.pid() == subscriber.pid()) {
        if let Err(error) = watch_target(WatchTarget::Subscriber(subscriber.clone())) {
          system.emit_log(
            LogLevel::Warn,
            alloc::format!("receptionist failed to watch subscriber: {:?}", error),
            origin,
            None,
          );
        }
        subscribers.push(subscriber.clone());
      }
    },
    | ReceptionistCommand::Unsubscribe { service_id, type_id, subscriber } => {
      let key = (service_id.clone(), *type_id);
      let mut remove_key = false;
      if let Some(subscribers) = state.subscribers.get_mut(&key) {
        subscribers.retain(|existing| existing.pid() != subscriber.pid());
        remove_key = subscribers.is_empty();
      }
      if remove_key {
        state.subscribers.remove(&key);
      }
    },
    | ReceptionistCommand::Find { service_id, type_id, reply_to } => {
      let key = (service_id.clone(), *type_id);
      let current = state.registrations.get(&key).cloned().unwrap_or_default();
      let listing = Listing::new(service_id.clone(), *type_id, current);
      let mut reply_to = reply_to.clone();
      reply_to.try_tell(listing).map_err(|error| ActorError::from_send_error(&error))?;
    },
  }

  Ok(())
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
) {
  if let Some(subs) = subscribers.get(key) {
    let refs = registrations.get(key).cloned().unwrap_or_default();
    let listing = Listing::new(key.0.clone(), key.1, refs);
    for sub in subs {
      let mut s = sub.clone();
      if let Err(e) = s.try_tell(listing.clone()) {
        system.emit_log(
          crate::core::kernel::event::logging::LogLevel::Warn,
          alloc::format!("receptionist failed to notify subscriber {:?}: {:?}", sub.pid(), e),
          None,
          None,
        );
      }
    }
  }
}
