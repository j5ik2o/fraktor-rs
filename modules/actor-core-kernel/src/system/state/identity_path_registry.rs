//! Identity and actor path state owned by SystemState.

#[cfg(test)]
#[path = "identity_path_registry_test.rs"]
mod tests;

use portable_atomic::AtomicU64;

use super::{ActorPathRegistry, ExtraTopLevels, TempActors, path_identity::PathIdentity};

/// Owns actor system identity and path-related state.
#[derive(Default)]
pub(crate) struct IdentityPathRegistry {
  pub(crate) path_identity:       PathIdentity,
  pub(crate) actor_path_registry: ActorPathRegistry,
  pub(crate) extra_top_levels:    ExtraTopLevels,
  pub(crate) temp_actors:         TempActors,
  pub(crate) temp_counter:        AtomicU64,
}
