//! Typed cluster sharding extension facade.

#[cfg(test)]
#[path = "cluster_sharding_test.rs"]
mod tests;

use alloc::vec;
use core::any::Any;

use fraktor_actor_core_kernel_rs::actor::extension::Extension;
use fraktor_actor_core_typed_rs::TypedActorSystem;
use fraktor_cluster_core_kernel_rs::{
  activation::{ActivatedKind, ClusterIdentityError, IdentitySetupError},
  extension::{ClusterApi, ClusterApiError, ClusterExtension},
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{Entity, entity_region::EntityRegion, grain_ref::GrainRef, grain_type_key::GrainTypeKey};

/// Typed facade for cluster sharding initialization and entity reference lookup.
///
/// This is the fraktor equivalent of Pekko's typed `ClusterSharding` extension.
/// Kind registration and reference resolution delegate to the kernel cluster
/// extension and grain APIs; no placement state machine is duplicated here.
pub struct ClusterSharding {
  api:       ClusterApi,
  extension: ArcShared<ClusterExtension>,
}

impl Extension for ClusterSharding {}

impl ClusterSharding {
  /// Retrieves the typed cluster sharding facade from a typed actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension has not been installed.
  pub fn get<M>(system: &TypedActorSystem<M>) -> Result<Self, ClusterApiError>
  where
    M: Send + Sync + 'static, {
    Self::try_from_system(system.as_untyped())
  }

  /// Retrieves the typed cluster sharding facade from an actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension has not been installed.
  pub fn try_from_system(system: &fraktor_actor_core_kernel_rs::system::ActorSystem) -> Result<Self, ClusterApiError> {
    let api = ClusterApi::try_from_system(system)?;
    let extension =
      system.extended().extension_by_type::<ClusterExtension>().ok_or(ClusterApiError::ExtensionNotInstalled)?;
    Ok(Self { api, extension })
  }

  /// Initializes sharding for the given entity declaration.
  ///
  /// Registers the entity kind via the kernel [`ClusterExtension::setup_member_kinds`]
  /// API and returns a typed resolution handle. The stored behavior factory placeholder
  /// is not invoked yet.
  ///
  /// # Errors
  ///
  /// Returns [`IdentitySetupError`] when kernel kind registration fails.
  pub fn init<M>(&self, entity: Entity<M>) -> Result<EntityRegion<M>, IdentitySetupError>
  where
    M: Any + Send + Sync + 'static, {
    let type_key = entity.into_type_key();
    self.register_kind(type_key.kind())?;
    Ok(EntityRegion::new(self.api.clone(), type_key))
  }

  /// Initializes sharding for the given grain type key.
  ///
  /// This is a convenience overload when no entity declaration wrapper is needed.
  ///
  /// # Errors
  ///
  /// Returns [`IdentitySetupError`] when kernel kind registration fails.
  pub fn init_type_key<M>(&self, type_key: GrainTypeKey<M>) -> Result<EntityRegion<M>, IdentitySetupError>
  where
    M: Any + Send + Sync + 'static, {
    self.register_kind(type_key.kind())?;
    Ok(EntityRegion::new(self.api.clone(), type_key))
  }

  /// Resolves a typed grain reference for the given type key and entity id.
  ///
  /// The caller must have already registered the kind, typically via [`Self::init`].
  ///
  /// # Errors
  ///
  /// Returns [`ClusterIdentityError`] when the entity id or kind is invalid.
  pub fn entity_ref_for<M>(
    &self,
    type_key: &GrainTypeKey<M>,
    entity_id: &str,
  ) -> Result<GrainRef<M>, ClusterIdentityError>
  where
    M: Any + Send + Sync + 'static, {
    let identity = type_key.identity_for(entity_id)?;
    Ok(crate::entity_region::grain_ref_for(&self.api, &identity))
  }

  fn register_kind(&self, kind: &str) -> Result<(), IdentitySetupError> {
    self.extension.setup_member_kinds(vec![ActivatedKind::new(kind)])
  }
}
