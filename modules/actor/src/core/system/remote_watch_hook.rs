//! Hook used by actor-ref providers to intercept remote watch/unwatch signals.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::actor_prim::Pid;

/// Allows custom providers to reroute `SystemMessage::Watch/Unwatch` for remote actors.
///
/// Implementations should be wrapped in a `ToolboxMutex` by callers to ensure thread-safe access.
/// The `&mut self` signature makes state changes explicit in the type system.
pub trait RemoteWatchHook<TB>: Send + 'static
where
  TB: RuntimeToolbox + 'static, {
  /// Handles a watch request. Returns `true` when the provider consumed the message.
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool;

  /// Handles an unwatch request. Returns `true` when the provider consumed the message.
  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool;
}
