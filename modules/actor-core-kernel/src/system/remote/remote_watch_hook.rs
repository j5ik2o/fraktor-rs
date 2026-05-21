//! Hook used by actor-ref providers to intercept remote watch/unwatch signals.

use crate::actor::{Pid, error::SendError};

/// Allows custom providers to reroute `SystemMessage::Watch/Unwatch` for remote actors.
///
/// Implementations should be wrapped in a `SpinSyncMutex` by callers to ensure thread-safe access.
/// The `&mut self` signature makes state changes explicit in the type system.
pub trait RemoteWatchHook: Send + 'static {
  /// Handles a watch request. Returns `true` when the provider consumed the message.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the provider owns the target but cannot enqueue the watch request.
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> Result<bool, SendError>;

  /// Handles an unwatch request. Returns `true` when the provider consumed the message.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the provider owns the target but cannot enqueue the unwatch
  /// request.
  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> Result<bool, SendError>;

  /// Handles a remote-bound termination notification. Returns `true` when the provider consumed the
  /// message.
  fn handle_deathwatch_notification(&mut self, watcher: Pid, terminated: Pid) -> bool;
}
