//! Resolved grain reference containing identity and actor ref.

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;

use crate::identity::ClusterIdentity;

/// Resolved grain reference with identity metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrainResolvedRef {
  /// Resolved cluster identity.
  pub identity:  ClusterIdentity,
  /// Resolved actor reference.
  pub actor_ref: ActorRef,
}
