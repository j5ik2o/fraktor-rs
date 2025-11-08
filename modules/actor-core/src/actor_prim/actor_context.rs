//! Actor execution context utilities.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};
use core::marker::PhantomData;

use crate::{
  NoStdToolbox, RuntimeToolbox,
  actor_prim::{ChildRefGeneric, Pid, actor_ref::ActorRefGeneric},
  error::SendError,
  logging::LogLevel,
  messaging::{AnyMessageGeneric, SystemMessage},
  props::PropsGeneric,
  spawn::SpawnError,
  system::ActorSystemGeneric,
};

/// Provides contextual APIs while handling a message.
pub struct ActorContextGeneric<'a, TB: RuntimeToolbox + 'static> {
  system:   ActorSystemGeneric<TB>,
  pid:      Pid,
  reply_to: Option<ActorRefGeneric<TB>>,
  _marker:  PhantomData<&'a ()>,
}

/// Alias for a context with the default runtime toolbox.
pub type ActorContext<'a> = ActorContextGeneric<'a, NoStdToolbox>;

impl<'a, TB: RuntimeToolbox + 'static> ActorContextGeneric<'a, TB> {
  /// Creates a new context placeholder.
  #[must_use]
  pub fn new(system: &ActorSystemGeneric<TB>, pid: Pid) -> Self {
    Self { system: system.clone(), pid, reply_to: None, _marker: PhantomData }
  }

  /// Returns a reference to the actor system.
  #[must_use]
  pub const fn system(&self) -> &ActorSystemGeneric<TB> {
    &self.system
  }

  /// Returns the pid of the running actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the reply target if supplied by the message envelope.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&ActorRefGeneric<TB>> {
    self.reply_to.as_ref()
  }

  /// Sets the reply target (used internally by the runtime).
  pub fn set_reply_to(&mut self, reply_to: Option<ActorRefGeneric<TB>>) {
    self.reply_to = reply_to;
  }

  /// Clears the reply target after message processing completes.
  pub fn clear_reply_to(&mut self) {
    self.reply_to = None;
  }

  /// Returns an [`ActorRef`] pointing to the running actor.
  ///
  /// # Panics
  ///
  /// Panics if the actor reference cannot be resolved.
  #[must_use]
  pub fn self_ref(&self) -> ActorRefGeneric<TB> {
    match self.system.actor_ref(self.pid) {
      | Some(reference) => reference,
      | None => panic!("actor reference must exist for running context"),
    }
  }

  /// Sends a reply to the caller if a reply target is present.
  ///
  /// # Errors
  ///
  /// Returns an error if no reply target is set or sending fails.
  pub fn reply(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    match self.reply_to.as_ref() {
      | Some(target) => target.tell(message),
      | None => Err(SendError::no_recipient(message)),
    }
  }

  /// Requests the actor system to spawn a child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when spawning the child fails.
  pub fn spawn_child(&self, props: &PropsGeneric<TB>) -> Result<ChildRefGeneric<TB>, SpawnError> {
    self.system.spawn_child(self.pid, props)
  }

  /// Returns the list of supervised children.
  #[must_use]
  pub fn children(&self) -> Vec<ChildRefGeneric<TB>> {
    self.system.children(self.pid)
  }

  /// Sends a stop signal to the specified child.
  ///
  /// # Errors
  ///
  /// Returns an error when the stop message cannot be delivered.
  pub fn stop_child(&self, child: &ChildRefGeneric<TB>) -> Result<(), SendError<TB>> {
    child.stop()
  }

  /// Sends a stop signal to the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the stop message cannot be delivered.
  pub fn stop_self(&self) -> Result<(), SendError<TB>> {
    self.system.stop_actor(self.pid)
  }

  /// Suspends the specified child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the suspend signal cannot be delivered.
  pub fn suspend_child(&self, child: &ChildRefGeneric<TB>) -> Result<(), SendError<TB>> {
    child.suspend()
  }

  /// Resumes the specified child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the resume signal cannot be delivered.
  pub fn resume_child(&self, child: &ChildRefGeneric<TB>) -> Result<(), SendError<TB>> {
    child.resume()
  }

  /// Subscribes the running actor to termination events for the specified target.
  ///
  /// # Errors
  ///
  /// Returns an error when the runtime cannot enqueue the watch signal.
  pub fn watch(&self, target: &ActorRefGeneric<TB>) -> Result<(), SendError<TB>> {
    if target.pid() == self.pid {
      return Ok(());
    }

    let state = self.system.state();
    match state.send_system_message(target.pid(), SystemMessage::Watch(self.pid)) {
      | Ok(()) => Ok(()),
      | Err(SendError::Closed(_)) => {
        let _ = state.send_system_message(self.pid, SystemMessage::Terminated(target.pid()));
        Ok(())
      },
      | Err(error) => Err(error),
    }
  }

  /// Stops watching the specified actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the runtime cannot enqueue the unwatch signal.
  pub fn unwatch(&self, target: &ActorRefGeneric<TB>) -> Result<(), SendError<TB>> {
    if target.pid() == self.pid {
      return Ok(());
    }

    let state = self.system.state();
    match state.send_system_message(target.pid(), SystemMessage::Unwatch(self.pid)) {
      | Ok(()) => Ok(()),
      | Err(SendError::Closed(_)) => Ok(()),
      | Err(error) => Err(error),
    }
  }

  /// Spawns a child actor and immediately starts monitoring it for termination.
  ///
  /// # Errors
  ///
  /// Returns an error when spawning fails or when installing the watch registration cannot be
  /// performed.
  pub fn spawn_child_watched(&self, props: &PropsGeneric<TB>) -> Result<ChildRefGeneric<TB>, SpawnError> {
    let child = self.spawn_child(props)?;
    if self.watch(child.actor_ref()).is_err() {
      let _ = child.stop();
      return Err(SpawnError::invalid_props("failed to install death watch"));
    }
    Ok(child)
  }

  /// Emits a log event associated with the running actor.
  pub fn log(&self, level: LogLevel, message: impl Into<String>) {
    self.system.emit_log(level, message.into(), Some(self.pid));
  }

  /// Pipes a message back to the running actor by reusing the mailbox on the same thread。
  ///
  /// # Errors
  ///
  /// Returns an error if sending the adapted message to the actor fails.
  pub fn pipe_to_self<F>(&self, message: AnyMessageGeneric<TB>, adapter: F) -> Result<(), SendError<TB>>
  where
    F: Fn(AnyMessageGeneric<TB>) -> AnyMessageGeneric<TB>, {
    let adapted = adapter(message);
    self.self_ref().tell(adapted)
  }
}
