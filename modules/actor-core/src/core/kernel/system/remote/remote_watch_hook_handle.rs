//! Handle wrapper for RemoteWatchHook implementations.

use super::{super::TerminationSignal, ActorRefProvider, RemoteWatchHook};
use crate::core::kernel::actor::{
  Pid,
  actor_path::{ActorPath, ActorPathScheme},
  actor_ref::ActorRef,
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
}

impl<P> RemoteWatchHook for RemoteWatchHookHandle<P>
where
  P: RemoteWatchHook,
{
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.provider.handle_watch(target, watcher)
  }

  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.provider.handle_unwatch(target, watcher)
  }
}

impl<P> ActorRefProvider for RemoteWatchHookHandle<P>
where
  P: ActorRefProvider,
{
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.supported_schemes()
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.provider.actor_ref(path)
  }

  fn termination_signal(&self) -> TerminationSignal {
    self.provider.termination_signal()
  }
}
