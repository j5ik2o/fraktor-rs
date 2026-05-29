//! Cluster identity resolution port.

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;

use crate::{ClusterResolveError, activation::ClusterIdentity};

/// Resolves a cluster identity into an actor reference.
pub trait ClusterIdentityResolver: Send + Sync {
  /// Resolves an identity into an actor reference.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster identity cannot be resolved.
  fn resolve(&self, identity: &ClusterIdentity) -> Result<ActorRef, ClusterResolveError>;
}
