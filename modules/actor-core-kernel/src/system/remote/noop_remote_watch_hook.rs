//! No-op implementation of [`RemoteWatchHook`].

use super::RemoteWatchHook;
use crate::actor::{Pid, error::SendError};

/// A no-op implementation of [`RemoteWatchHook`] that always returns `false`.
///
/// This is used as the default hook when no remote watch handling is configured.
pub(crate) struct NoopRemoteWatchHook;

impl RemoteWatchHook for NoopRemoteWatchHook {
  fn handle_watch(&mut self, _target: Pid, _watcher: Pid) -> Result<bool, SendError> {
    Ok(false)
  }

  fn handle_unwatch(&mut self, _target: Pid, _watcher: Pid) -> Result<bool, SendError> {
    Ok(false)
  }

  fn handle_deathwatch_notification(&mut self, _watcher: Pid, _terminated: Pid) -> bool {
    false
  }
}
