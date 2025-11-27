//! Shared wrapper for RemoteWatchHook implementations.

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{ActorRefProvider, RemoteWatchHook};
use crate::core::{
  actor_prim::{Pid, actor_path::ActorPathScheme, actor_ref::ActorRefGeneric},
  error::ActorError,
};

/// Shared wrapper that provides thread-safe access to a provider implementing
/// both [`ActorRefProvider`] and [`RemoteWatchHook`].
///
/// This adapter wraps a provider in a `ToolboxMutex`, allowing it to be shared
/// across multiple owners while satisfying the `&mut self` requirement of
/// `RemoteWatchHook` methods.
///
/// # Usage
///
/// 1. Create a shared wrapper: `RemoteWatchHookShared::new(provider,
///    &[ActorPathScheme::FraktorTcp])`
/// 2. Clone and wrap in `ArcShared` for `ActorRefProvider` registration
/// 3. Pass the original shared instance for `RemoteWatchHook` registration
pub struct RemoteWatchHookShared<TB: RuntimeToolbox + 'static, P: Send + 'static> {
  inner:   ArcShared<ToolboxMutex<P, TB>>,
  schemes: &'static [ActorPathScheme],
}

impl<TB: RuntimeToolbox + 'static, P: Send + 'static> RemoteWatchHookShared<TB, P> {
  /// Creates a new shared wrapper around the provided implementation.
  ///
  /// The `schemes` parameter specifies the actor path schemes supported by
  /// the underlying provider for `ActorRefProvider::supported_schemes()`.
  pub fn new(provider: P, schemes: &'static [ActorPathScheme]) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(provider)), schemes }
  }

  /// Returns a reference to the inner shared mutex.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<ToolboxMutex<P, TB>> {
    &self.inner
  }
}

impl<TB: RuntimeToolbox + 'static, P: Send + 'static> Clone for RemoteWatchHookShared<TB, P> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), schemes: self.schemes }
  }
}

impl<TB: RuntimeToolbox + 'static, P: RemoteWatchHook<TB> + Send + 'static> RemoteWatchHook<TB>
  for RemoteWatchHookShared<TB, P>
{
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.inner.lock().handle_watch(target, watcher)
  }

  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.inner.lock().handle_unwatch(target, watcher)
  }
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + RemoteWatchHook<TB> + Send + 'static> ActorRefProvider<TB>
  for RemoteWatchHookShared<TB, P>
{
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.schemes
  }

  fn actor_ref(&self, path: crate::core::actor_prim::actor_path::ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    self.inner.lock().actor_ref(path)
  }
}
