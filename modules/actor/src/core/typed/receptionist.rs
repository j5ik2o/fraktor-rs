//! Receptionist actor providing service discovery within an actor system.

mod listing;
mod receptionist_command;
mod service_key;
#[cfg(test)]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::any::TypeId;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};
pub use listing::Listing;
pub use receptionist_command::ReceptionistCommand;
pub use service_key::ServiceKey;

use crate::core::{
  kernel::actor::{actor_ref::ActorRef, error::ActorError},
  typed::{TypedActorRef, behavior::Behavior, message_and_signals::BehaviorSignal, dsl::Behaviors},
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

/// Receptionist actor that manages service registrations and subscriptions.
///
/// Use [`Receptionist::behavior`] to obtain the initial behavior for the receptionist actor.
/// Interact with it by sending [`ReceptionistCommand`] messages via its `TypedActorRef`.
pub struct Receptionist;

impl Receptionist {
  /// Returns the initial behavior for the Receptionist actor.
  #[must_use]
  pub fn behavior() -> Behavior<ReceptionistCommand> {
    let state = ArcShared::new(RuntimeMutex::new(ReceptionistState {
      registrations: BTreeMap::new(),
      subscribers:   BTreeMap::new(),
    }));
    let state_for_message = state.clone();
    let state_for_signal = state;

    Behaviors::receive_message(move |ctx, cmd| {
      let mut guard = state_for_message.lock();
      match cmd {
        | ReceptionistCommand::Register { service_id, type_id, actor_ref } => {
          let key = (service_id.clone(), *type_id);
          let entry = guard.registrations.entry(key.clone()).or_default();
          if !entry.iter().any(|r| r.pid() == actor_ref.pid()) {
            entry.push(actor_ref.clone());
            if let Err(e) = ctx.as_untyped_mut().watch(actor_ref) {
              ctx.system().emit_log(
                crate::core::kernel::event::logging::LogLevel::Warn,
                alloc::format!("receptionist failed to watch registered actor: {:?}", e),
                Some(ctx.pid()),
              );
            }
            notify_subscribers(&guard.subscribers, &key, &guard.registrations);
          }
        },
        | ReceptionistCommand::Deregister { service_id, type_id, actor_ref } => {
          let key = (service_id.clone(), *type_id);
          if let Some(entry) = guard.registrations.get_mut(&key) {
            let before = entry.len();
            entry.retain(|r| r.pid() != actor_ref.pid());
            if entry.len() != before {
              notify_subscribers(&guard.subscribers, &key, &guard.registrations);
            }
          }
        },
        | ReceptionistCommand::Subscribe { service_id, type_id, subscriber } => {
          let key = (service_id.clone(), *type_id);
          let current = guard.registrations.get(&key).cloned().unwrap_or_default();
          let listing = Listing::new(service_id.clone(), *type_id, current);
          let mut sub = subscriber.clone();
          sub.try_tell(listing).map_err(|error| ActorError::from_send_error(&error))?;
          let subscribers = guard.subscribers.entry(key).or_default();
          if !subscribers.iter().any(|existing| existing.pid() == subscriber.pid()) {
            if let Err(e) = ctx.watch(subscriber) {
              ctx.system().emit_log(
                crate::core::kernel::event::logging::LogLevel::Warn,
                alloc::format!("receptionist failed to watch subscriber: {:?}", e),
                Some(ctx.pid()),
              );
            }
            subscribers.push(subscriber.clone());
          }
        },
        | ReceptionistCommand::Unsubscribe { service_id, type_id, subscriber } => {
          let key = (service_id.clone(), *type_id);
          let mut remove_key = false;
          if let Some(subscribers) = guard.subscribers.get_mut(&key) {
            subscribers.retain(|existing| existing.pid() != subscriber.pid());
            remove_key = subscribers.is_empty();
          }
          if remove_key {
            guard.subscribers.remove(&key);
          }
        },
        | ReceptionistCommand::Find { service_id, type_id, reply_to } => {
          let key = (service_id.clone(), *type_id);
          let current = guard.registrations.get(&key).cloned().unwrap_or_default();
          let listing = Listing::new(service_id.clone(), *type_id, current);
          let mut reply = reply_to.clone();
          reply.try_tell(listing).map_err(|error| ActorError::from_send_error(&error))?;
        },
      }
      Ok(Behaviors::same())
    })
    .receive_signal(move |_ctx, signal| {
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
        notify_subscribers(&guard.subscribers, key, &guard.registrations);
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

/// Notifies all subscribers of a key about the current registration set.
fn notify_subscribers(
  subscribers: &BTreeMap<RegistryKey, Vec<TypedActorRef<Listing>>>,
  key: &RegistryKey,
  registrations: &BTreeMap<RegistryKey, Vec<ActorRef>>,
) {
  if let Some(subs) = subscribers.get(key) {
    let refs = registrations.get(key).cloned().unwrap_or_default();
    let listing = Listing::new(key.0.clone(), key.1, refs);
    for sub in subs {
      let mut s = sub.clone();
      if let Err(_error) = s.try_tell(listing.clone()) {}
    }
  }
}
