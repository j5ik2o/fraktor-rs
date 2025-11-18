//! Hook used by actor-ref providers to intercept remote watch/unwatch signals.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::actor_prim::Pid;

/// Allows custom providers to reroute `SystemMessage::Watch/Unwatch` for remote actors.
pub trait RemoteWatchHook<TB>: Send + Sync + 'static
where
  TB: RuntimeToolbox + 'static, {
  /// Handles a watch request. Returns `true` when the provider consumed the message.
  fn handle_watch(&self, target: Pid, watcher: Pid) -> bool;

  /// Handles an unwatch request. Returns `true` when the provider consumed the message.
  fn handle_unwatch(&self, target: Pid, watcher: Pid) -> bool;
}
