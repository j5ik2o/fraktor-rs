//! No-op implementation of [`RemoteWatchHook`].

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::RemoteWatchHook;
use crate::core::actor_prim::Pid;

/// A no-op implementation of [`RemoteWatchHook`] that always returns `false`.
///
/// This is used as the default hook when no remote watch handling is configured.
pub(crate) struct NoopRemoteWatchHook;

impl<TB: RuntimeToolbox + 'static> RemoteWatchHook<TB> for NoopRemoteWatchHook {
  fn handle_watch(&mut self, _target: Pid, _watcher: Pid) -> bool {
    false
  }

  fn handle_unwatch(&mut self, _target: Pid, _watcher: Pid) -> bool {
    false
  }
}
