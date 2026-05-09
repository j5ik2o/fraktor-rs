//! Acknowledgement type sent when an actor is registered with the receptionist.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::TypeId;

use fraktor_actor_core_kernel_rs::actor::{actor_ref::ActorRef, error::ActorError};

use super::service_key::ServiceKey;
use crate::TypedActorRef;

/// Acknowledgement sent when an actor is successfully registered with the receptionist.
///
/// Corresponds to Pekko's `Receptionist.Registered`.
#[derive(Clone, Debug)]
pub struct Registered {
  service_id: String,
  type_id:    TypeId,
  actor_ref:  ActorRef,
}

impl Registered {
  /// Creates a new `Registered` acknowledgement.
  #[must_use]
  pub fn new(service_id: impl Into<String>, type_id: TypeId, actor_ref: ActorRef) -> Self {
    Self { service_id: service_id.into(), type_id, actor_ref }
  }

  /// Returns the service identifier.
  #[must_use]
  pub fn service_id(&self) -> &str {
    &self.service_id
  }

  /// Returns the type identifier of the message type.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Returns whether this acknowledgement matches the given service key.
  ///
  /// Corresponds to Pekko's `Registered.isForKey`.
  #[must_use]
  pub fn is_for_key<M>(&self, key: &ServiceKey<M>) -> bool
  where
    M: Send + Sync + 'static, {
    self.service_id == key.id() && self.type_id == key.type_id()
  }

  /// Returns a typed actor reference for the registered actor.
  ///
  /// Corresponds to Pekko's `Registered.serviceInstance`.
  ///
  /// # Errors
  ///
  /// Returns an error when `M` does not match the registration type.
  pub fn service_instance<M>(&self, key: &ServiceKey<M>) -> Result<TypedActorRef<M>, ActorError>
  where
    M: Send + Sync + 'static, {
    if !self.is_for_key(key) {
      return Err(ActorError::recoverable("registered key mismatch"));
    }
    Ok(TypedActorRef::from_untyped(self.actor_ref.clone()))
  }
}
