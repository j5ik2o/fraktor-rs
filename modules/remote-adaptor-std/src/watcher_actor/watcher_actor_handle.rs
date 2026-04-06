//! Handle returned by [`crate::watcher_actor::WatcherActor::spawn`].

use alloc::boxed::Box;

use fraktor_remote_core_rs::watcher::WatcherCommand;
use tokio::sync::mpsc;

/// Boxed [`WatcherCommand`] returned in the `Err` arm of
/// [`WatcherActorHandle::submit`] when the actor task has exited.
///
/// Boxing keeps the `Err` variant small (`clippy::result_large_err`).
pub type SubmitError = Box<WatcherCommand>;

/// Handle returned by [`crate::watcher_actor::WatcherActor::spawn`] for
/// submitting commands to the running actor task.
#[derive(Debug, Clone)]
pub struct WatcherActorHandle {
  command_tx: mpsc::UnboundedSender<WatcherCommand>,
}

impl WatcherActorHandle {
  /// Internal constructor used by [`crate::watcher_actor::WatcherActor`].
  #[must_use]
  pub(crate) const fn new(command_tx: mpsc::UnboundedSender<WatcherCommand>) -> Self {
    Self { command_tx }
  }

  /// Submits a [`WatcherCommand`] to the actor.
  ///
  /// # Errors
  ///
  /// Returns the original command (boxed) when the actor task has already
  /// exited and the receiver has been dropped.
  pub fn submit(&self, command: WatcherCommand) -> Result<(), SubmitError> {
    self.command_tx.send(command).map_err(|err| Box::new(err.0))
  }
}
