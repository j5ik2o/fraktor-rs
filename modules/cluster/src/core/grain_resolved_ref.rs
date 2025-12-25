//! Resolved grain reference containing identity and actor ref.

use fraktor_actor_rs::core::actor_prim::actor_ref::ActorRefGeneric;
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::ClusterIdentity;

/// Resolved grain reference with identity metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrainResolvedRef<TB: RuntimeToolbox + 'static> {
  /// Resolved cluster identity.
  pub identity:  ClusterIdentity,
  /// Resolved actor reference.
  pub actor_ref: ActorRefGeneric<TB>,
}
