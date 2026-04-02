//! Snapshot of actor references registered under a service key.

#[cfg(test)]
mod tests;

use alloc::{collections::BTreeSet, string::String, vec::Vec};
use core::any::TypeId;

use super::service_key::ServiceKey;
use crate::core::{kernel::actor::error::ActorError, typed::TypedActorRef};

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

  /// Returns whether this listing was produced for the given service key.
  ///
  /// Corresponds to Pekko's `Listing.isForKey`.
  #[must_use]
  pub fn is_for_key<M>(&self, key: &ServiceKey<M>) -> bool
  where
    M: Send + Sync + 'static, {
    self.service_id == key.id() && self.type_id == key.type_id()
  }

  /// Returns typed actor references for the given service key.
  ///
  /// Corresponds to Pekko's `Listing.serviceInstances`.
  ///
  /// # Errors
  ///
  /// Returns an error when the key does not match this listing.
  pub fn service_instances<M>(&self, key: &ServiceKey<M>) -> Result<BTreeSet<TypedActorRef<M>>, ActorError>
  where
    M: Send + Sync + 'static, {
    if !self.is_for_key(key) {
      return Err(ActorError::recoverable("listing key mismatch"));
    }
    Ok(self.typed_refs::<M>()?.into_iter().collect())
  }

  /// Returns all typed actor references for the given service key.
  ///
  /// In the current non-clustered implementation this is identical to
  /// [`Self::service_instances`].
  ///
  /// # Errors
  ///
  /// Returns an error when the key does not match this listing.
  pub fn all_service_instances<M>(&self, key: &ServiceKey<M>) -> Result<BTreeSet<TypedActorRef<M>>, ActorError>
  where
    M: Send + Sync + 'static, {
    self.service_instances(key)
  }

  /// Returns whether the listing reflects added or removed services.
  // TODO: Track actual add/remove diffs when clustered receptionist reachability
  // semantics are introduced. The current local-only implementation mirrors
  // Pekko's non-clustered contract and therefore always returns true.
  #[must_use]
  pub const fn services_were_added_or_removed(&self) -> bool {
    true
  }
}
