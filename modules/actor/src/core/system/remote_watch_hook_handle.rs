//! Handle wrapper for RemoteWatchHook implementations.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{ActorRefProvider, RemoteWatchHook};
use crate::core::{
  actor::{Pid, actor_path::ActorPathScheme, actor_ref::ActorRefGeneric},
  error::ActorError,
};

/// Handle wrapper that combines a provider with its supported schemes.
///
/// This struct stores a static reference to the supported schemes, avoiding
/// repeated calls to `supported_schemes()` on the inner provider.
pub struct RemoteWatchHookHandle<P> {
  provider: P,
  schemes:  &'static [ActorPathScheme],
}

impl<P> RemoteWatchHookHandle<P> {
  pub(crate) const fn new(provider: P, schemes: &'static [ActorPathScheme]) -> Self {
    Self { provider, schemes }
  }

  const fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.schemes
  }

  /// Returns a reference to the inner provider.
  ///
  /// This method is intended for testing and debugging purposes only.
  #[doc(hidden)]
  pub const fn inner(&self) -> &P {
    &self.provider
  }

  /// Returns a mutable reference to the inner provider.
  ///
  /// This method is intended for testing and debugging purposes only.
  #[doc(hidden)]
  pub const fn inner_mut(&mut self) -> &mut P {
    &mut self.provider
  }
}

impl<TB, P> RemoteWatchHook<TB> for RemoteWatchHookHandle<P>
where
  TB: RuntimeToolbox + 'static,
  P: RemoteWatchHook<TB>,
{
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.provider.handle_watch(target, watcher)
  }

  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.provider.handle_unwatch(target, watcher)
  }
}

impl<TB, P> ActorRefProvider<TB> for RemoteWatchHookHandle<P>
where
  TB: RuntimeToolbox + 'static,
  P: ActorRefProvider<TB>,
{
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.supported_schemes()
  }

  fn actor_ref(&mut self, path: crate::core::actor::actor_path::ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    self.provider.actor_ref(path)
  }
}
