//! Handle returned by [`ClusterSharding::init`](crate::ClusterSharding::init) for typed entity
//! reference resolution.

use core::any::Any;

use fraktor_cluster_core_kernel_rs::{activation::ClusterIdentityError, extension::ClusterApi};

use crate::{ClusterIdentity, GrainRef, GrainTypeKey};

/// Handle returned by [`ClusterSharding::init`](crate::ClusterSharding::init) for typed entity
/// reference resolution.
pub struct EntityRegion<M> {
  api:      ClusterApi,
  type_key: GrainTypeKey<M>,
}

impl<M> EntityRegion<M> {
  pub(crate) const fn new(api: ClusterApi, type_key: GrainTypeKey<M>) -> Self {
    Self { api, type_key }
  }

  /// Returns the registered grain type key for this region.
  #[must_use]
  pub const fn type_key(&self) -> &GrainTypeKey<M> {
    &self.type_key
  }

  /// Resolves a typed grain reference for the given entity id.
  ///
  /// Delegates identity construction to [`GrainTypeKey::identity_for`] and reference
  /// construction to the kernel [`ClusterApi`] / [`GrainRef`] path.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterIdentityError`] when the entity id or stored kind is invalid.
  pub fn entity_ref_for(&self, entity_id: &str) -> Result<GrainRef<M>, ClusterIdentityError>
  where
    M: Any + Send + Sync + 'static, {
    let identity = self.type_key.identity_for(entity_id)?;
    Ok(grain_ref_for(&self.api, &identity))
  }
}

pub(crate) fn grain_ref_for<M>(api: &ClusterApi, identity: &ClusterIdentity<M>) -> GrainRef<M>
where
  M: Any + Send + Sync + 'static, {
  let kernel_ref = fraktor_cluster_core_kernel_rs::grain::GrainRef::new(api.clone(), identity.as_kernel().clone());
  GrainRef::from_kernel(kernel_ref)
}
