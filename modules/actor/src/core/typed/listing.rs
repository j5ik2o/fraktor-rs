//! Snapshot of actor references registered under a service key.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};
use core::any::TypeId;

use crate::core::typed::actor::TypedActorRef;

/// A snapshot of actor references registered under a service key.
#[derive(Clone, Debug)]
pub struct Listing {
  service_id: String,
  type_id:    TypeId,
  refs:       Vec<crate::core::actor::actor_ref::ActorRef>,
}

impl Listing {
  /// Creates a new listing.
  #[must_use]
  pub fn new(
    service_id: impl Into<String>,
    type_id: TypeId,
    refs: Vec<crate::core::actor::actor_ref::ActorRef>,
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
  pub fn refs(&self) -> &[crate::core::actor::actor_ref::ActorRef] {
    &self.refs
  }

  /// Returns typed actor references by transmuting the erased references.
  ///
  /// # Safety
  ///
  /// The caller must ensure that the message type `M` matches the type used
  /// during registration.
  #[must_use]
  pub fn typed_refs<M>(&self) -> Vec<TypedActorRef<M>>
  where
    M: Send + Sync + 'static, {
    self.refs.iter().map(|r| TypedActorRef::from_untyped(r.clone())).collect()
  }

  /// Returns whether the listing is empty.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.refs.is_empty()
  }
}
