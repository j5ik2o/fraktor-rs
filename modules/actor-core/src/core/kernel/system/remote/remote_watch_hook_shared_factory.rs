//! Factory contract for shared remote-watch-hook handles.

use super::{ActorRefProvider, RemoteWatchHook, RemoteWatchHookHandleShared};

/// Materializes a shared remote-watch-hook handle using the selected lock family.
pub trait RemoteWatchHookHandleSharedFactory<P>: Send + Sync
where
  P: ActorRefProvider + RemoteWatchHook + 'static, {
  /// Wraps the provider into a shared remote-watch-hook handle.
  fn create_remote_watch_hook_handle_shared(&self, provider: P) -> RemoteWatchHookHandleShared<P>;
}
