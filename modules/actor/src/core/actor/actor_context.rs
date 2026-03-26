//! Actor execution context utilities.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::BTreeSet, string::String, vec::Vec};
use core::{future::Future, marker::PhantomData};

use crate::core::{
  actor::{ChildRef, Pid, actor_ref::ActorRef, pipe_spawn_error::PipeSpawnError},
  error::{ActorError, SendError},
  event::logging::LogLevel,
  messaging::{AnyMessage, system_message::SystemMessage},
  props::Props,
  spawn::SpawnError,
  system::ActorSystem,
};

pub(crate) const STASH_OVERFLOW_REASON: &str = "stash buffer overflow";

/// Provides contextual APIs while handling a message.
pub struct ActorContext<'a> {
  system:          ActorSystem,
  pid:             Pid,
  sender:          Option<ActorRef>,
  current_message: Option<AnyMessage>,
  _marker:         PhantomData<&'a ()>,
}

/// Alias for a context with the default runtime toolbox.
impl ActorContext<'_> {
  /// Creates a new context placeholder.
  #[must_use]
  pub fn new(system: &ActorSystem, pid: Pid) -> Self {
    Self { system: system.clone(), pid, sender: None, current_message: None, _marker: PhantomData }
  }

  /// Returns a reference to the actor system.
  #[must_use]
  pub const fn system(&self) -> &ActorSystem {
    &self.system
  }

  /// Returns the pid of the running actor.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the sender if supplied by the message envelope.
  #[must_use]
  pub const fn sender(&self) -> Option<&ActorRef> {
    self.sender.as_ref()
  }

  /// Sets the sender (used internally by the runtime).
  pub fn set_sender(&mut self, sender: Option<ActorRef>) {
    self.sender = sender;
  }

  /// Clears the sender after message processing completes.
  pub fn clear_sender(&mut self) {
    self.sender = None;
  }

  /// Sets the current user message being processed (runtime use only).
  pub(crate) fn set_current_message(&mut self, message: Option<AnyMessage>) {
    self.current_message = message;
  }

  /// Clears the current message marker after processing completes.
  pub(crate) fn clear_current_message(&mut self) {
    self.current_message = None;
  }

  /// Stashes the currently processed user message for deferred handling.
  ///
  /// # Errors
  ///
  /// Returns an error when no current message is active or when the actor cell is unavailable.
  pub fn stash(&mut self) -> Result<(), ActorError> {
    self.stash_with_limit(usize::MAX)
  }

  /// Stashes the currently processed user message with an explicit stash limit.
  ///
  /// # Errors
  ///
  /// Returns an error when no current message is active, when the stash reached `max_messages`,
  /// or when the actor cell is unavailable.
  pub fn stash_with_limit(&mut self, max_messages: usize) -> Result<(), ActorError> {
    let message = self
      .current_message
      .as_ref()
      .cloned()
      .ok_or_else(|| ActorError::recoverable("stash requires an active user message"))?;
    let cell = self
      .system
      .state()
      .cell(&self.pid)
      .ok_or_else(|| ActorError::recoverable("actor cell unavailable during stash"))?;
    cell.stash_message_with_limit(message, max_messages)
  }

  /// Returns true when the provided error is caused by stash capacity overflow.
  #[must_use]
  pub fn is_stash_overflow_error(error: &ActorError) -> bool {
    matches!(error, ActorError::Recoverable(reason) if reason.as_str() == STASH_OVERFLOW_REASON)
  }

  /// Re-enqueues the oldest previously stashed message back to this actor mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable or unstash dispatch fails.
  pub fn unstash(&self) -> Result<usize, ActorError> {
    let cell = self
      .system
      .state()
      .cell(&self.pid)
      .ok_or_else(|| ActorError::recoverable("actor cell unavailable during unstash"))?;
    cell.unstash_message()
  }

  /// Re-enqueues all previously stashed messages back to this actor mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cell is unavailable or unstash dispatch fails.
  pub fn unstash_all(&self) -> Result<usize, ActorError> {
    let cell = self
      .system
      .state()
      .cell(&self.pid)
      .ok_or_else(|| ActorError::recoverable("actor cell unavailable during unstash"))?;
    cell.unstash_messages()
  }

  /// Returns an [`ActorRef`] pointing to the running actor.
  ///
  /// # Panics
  ///
  /// Panics if the actor reference cannot be resolved.
  #[must_use]
  pub fn self_ref(&self) -> ActorRef {
    match self.system.actor_ref(self.pid) {
      | Some(reference) => reference,
      | None => panic!("actor reference must exist for running context"),
    }
  }

  /// Sends a reply to the caller if a reply target is present.
  ///
  /// This forwards the result of `try_tell` on the current sender.
  ///
  /// # Errors
  ///
  /// Returns an error if no reply target is set or if the reply message cannot
  /// be enqueued.
  pub fn reply(&mut self, message: AnyMessage) -> Result<(), SendError> {
    match self.sender.as_mut() {
      | Some(target) => target.try_tell(message),
      | None => Err(SendError::no_recipient(message)),
    }
  }

  /// Requests the actor system to spawn a child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when spawning the child fails.
  pub fn spawn_child(&mut self, props: &Props) -> Result<ChildRef, SpawnError> {
    self.system.spawn_child(self.pid, props)
  }

  /// Returns the list of supervised children.
  #[must_use]
  pub fn children(&self) -> Vec<ChildRef> {
    self.system.children(self.pid)
  }

  /// Returns the child with the specified name, if present.
  #[must_use]
  pub fn child(&self, name: &str) -> Option<ChildRef> {
    let state = self.system.state();
    state.child_pids(self.pid).into_iter().find_map(|pid| {
      let cell = state.cell(&pid)?;
      if cell.name() == name { Some(ChildRef::new(cell.actor_ref(), state.clone())) } else { None }
    })
  }

  /// Sends a stop signal to the specified child.
  ///
  /// # Errors
  ///
  /// Returns an error when the stop message cannot be delivered.
  pub fn stop_child(&mut self, child: &ChildRef) -> Result<(), SendError> {
    child.stop()
  }

  /// Sends a stop signal to the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the stop message cannot be delivered.
  pub fn stop_self(&mut self) -> Result<(), SendError> {
    self.system.stop_actor(self.pid)
  }

  /// Suspends the specified child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the suspend signal cannot be delivered.
  pub fn suspend_child(&mut self, child: &ChildRef) -> Result<(), SendError> {
    child.suspend()
  }

  /// Resumes the specified child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when the resume signal cannot be delivered.
  pub fn resume_child(&mut self, child: &ChildRef) -> Result<(), SendError> {
    child.resume()
  }

  /// Subscribes the running actor to termination events for the specified target.
  ///
  /// # Errors
  ///
  /// Returns an error when the runtime cannot enqueue the watch signal.
  pub fn watch(&mut self, target: &ActorRef) -> Result<(), SendError> {
    if target.pid() == self.pid {
      return Ok(());
    }

    let state = self.system.state();
    match state.send_system_message(target.pid(), SystemMessage::Watch(self.pid)) {
      | Ok(()) => Ok(()),
      | Err(SendError::Closed(_)) => {
        // Best-effort: target is already closed, so notify self about termination.
        if let Err(error) = state.send_system_message(self.pid, SystemMessage::Terminated(target.pid())) {
          state.record_send_error(Some(self.pid), &error);
        }
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
  pub fn unwatch(&mut self, target: &ActorRef) -> Result<(), SendError> {
    if target.pid() == self.pid {
      return Ok(());
    }

    let state = self.system.state();
    if let Some(cell) = state.cell(&self.pid) {
      cell.remove_watch_with(target.pid());
    }
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
  pub fn spawn_child_watched(&mut self, props: &Props) -> Result<ChildRef, SpawnError> {
    let child = self.spawn_child(props)?;
    if self.watch(child.actor_ref()).is_err() {
      // Best-effort stop: watch failed so the child must be cleaned up.
      if let Err(error) = child.stop() {
        self.system.state().record_send_error(Some(child.actor_ref().pid()), &error);
      }
      return Err(SpawnError::invalid_props("failed to install death watch"));
    }
    Ok(child)
  }

  /// Watches the specified target and delivers a custom message on termination.
  ///
  /// When the target terminates, the provided `message` is delivered as a user message
  /// instead of a `Terminated` signal.
  ///
  /// # Errors
  ///
  /// Returns an error when the runtime cannot enqueue the watch signal.
  pub fn watch_with(&mut self, target: &ActorRef, message: AnyMessage) -> Result<(), SendError> {
    if target.pid() == self.pid {
      return Ok(());
    }
    let state = self.system.state();
    let cell = state.cell(&self.pid).ok_or_else(|| SendError::no_recipient(message.clone()))?;
    cell.register_watch_with(target.pid(), message);
    if let Err(error) = self.watch(target) {
      // watch 失敗時はカスタムメッセージ登録をロールバックする
      cell.remove_watch_with(target.pid());
      return Err(error);
    }
    Ok(())
  }

  /// Forwards a message to the target, preserving the current sender.
  ///
  /// This is the user-facing fire-and-forget variant. Synchronous forwarding
  /// failures are observed internally and recorded via the system's send-error
  /// observation path.
  pub fn forward(&mut self, target: &mut ActorRef, message: AnyMessage) {
    let result = self.try_forward(target, message);
    if let Err(error) = result {
      self.system.state().record_send_error(Some(target.pid()), &error);
    }
  }

  /// Forwards the given message to the target, preserving the current sender.
  ///
  /// This mirrors Pekko's `ActorRef.forward`. The message is sent with the
  /// original sender of the currently processed message so the final recipient
  /// can reply to the original requester.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when forwarding fails synchronously while
  /// enqueueing the message into the target mailbox.
  pub fn try_forward(&mut self, target: &mut ActorRef, message: AnyMessage) -> Result<(), SendError> {
    let envelope = match &self.sender {
      | Some(sender) => message.with_sender(sender.clone()),
      | None => message,
    };
    target.try_tell(envelope)
  }

  /// Returns the metadata tags associated with the running actor.
  ///
  /// Returns an empty set if the actor cell is unavailable.
  #[must_use]
  pub fn tags(&self) -> BTreeSet<String> {
    self.system.state().cell(&self.pid).map(|cell| cell.tags().clone()).unwrap_or_default()
  }

  /// Emits a log event associated with the running actor.
  pub fn log(&self, level: LogLevel, message: impl Into<String>) {
    self.system.emit_log(level, message.into(), Some(self.pid));
  }

  /// Pipes the completion of an asynchronous computation back to the running actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor is unavailable or already stopped.
  pub fn pipe_to_self<Fut, Map, Output>(&mut self, future: Fut, map: Map) -> Result<(), PipeSpawnError>
  where
    Fut: Future<Output = Output> + Send + 'static,
    Map: FnOnce(Output) -> AnyMessage + Send + 'static, {
    let state = self.system.state();
    let Some(cell) = state.cell(&self.pid) else {
      return Err(PipeSpawnError::ActorUnavailable);
    };

    let mapped = async move {
      let value = future.await;
      map(value)
    };

    cell.spawn_pipe_task(Box::pin(mapped))
  }
}
