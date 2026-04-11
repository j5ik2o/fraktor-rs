//! Factory contract for shared actor-ref-provider handles.

use super::{ActorRefProvider, ActorRefProviderShared};

/// Materializes a shared actor-ref-provider handle using the selected lock family.
pub trait ActorRefProviderHandleSharedFactory<P>: Send + Sync
where
  P: ActorRefProvider + 'static, {
  /// Wraps the provider into a shared handle.
  fn create(&self, provider: P) -> ActorRefProviderShared<P>;
}
