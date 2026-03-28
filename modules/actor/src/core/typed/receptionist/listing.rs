//! Snapshot of actor references registered under a service key.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};
use core::any::TypeId;

use crate::core::{kernel::error::ActorError, typed::actor::TypedActorRef};

/// A snapshot of actor references registered under a service key.
#[derive(Clone, Debug)]
pub struct Listing {
  service_id: String,
  type_id:    TypeId,
  refs:       Vec<crate::core::kernel::actor::actor_ref::ActorRef>,
}

impl Listing {
  /// Creates a new listing.
  #[must_use]
  pub fn new(
    service_id: impl Into<String>,
    type_id: TypeId,
    refs: Vec<crate::core::kernel::actor::actor_ref::ActorRef>,
  ) -> Self {
    Self { service_id: service_id.into(), type_id, refs }
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

  /// Returns the registered actor references.
  #[must_use]
  pub fn refs(&self) -> &[crate::core::kernel::actor::actor_ref::ActorRef] {
    &self.refs
  }

  /// Returns typed actor references after validating the requested message type.
  ///
  /// # Errors
  ///
  /// Returns an error when `M` does not match the listing key type.
  pub fn typed_refs<M>(&self) -> Result<Vec<TypedActorRef<M>>, ActorError>
  where
    M: Send + Sync + 'static, {
    if self.type_id != TypeId::of::<M>() {
      return Err(ActorError::recoverable("listing type mismatch"));
    }
    Ok(self.refs.iter().map(|r| TypedActorRef::from_untyped(r.clone())).collect())
  }

  /// Returns whether the listing is empty.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.refs.is_empty()
  }
}
