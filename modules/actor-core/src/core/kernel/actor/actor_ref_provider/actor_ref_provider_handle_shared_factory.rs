//! Factory contract for shared actor-ref-provider handles.

use fraktor_utils_core_rs::core::sync::SharedLock;

use super::{ActorRefProvider, ActorRefProviderHandle, ActorRefProviderHandleShared};

/// Materializes a shared actor-ref-provider handle using the selected lock family.
pub trait ActorRefProviderHandleSharedFactory<P>: Send + Sync
where
  P: ActorRefProvider + 'static, {
  /// Wraps the provider into a shared handle.
  fn create_actor_ref_provider_handle_shared(&self, provider: P) -> ActorRefProviderHandleShared<P>;

  /// Wraps an already materialized shared lock in a shared handle.
  fn create_actor_ref_provider_handle_shared_from_shared(
    &self,
    shared: SharedLock<ActorRefProviderHandle<P>>,
  ) -> ActorRefProviderHandleShared<P>;
}
