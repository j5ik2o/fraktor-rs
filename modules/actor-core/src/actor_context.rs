//! Execution context handed to actors during lifecycle callbacks.

use crate::{
  actor_error::ActorError, actor_ref::ActorRef, any_message::AnyOwnedMessage, pid::Pid, props::Props,
  send_error::SendError,
};

type SpawnChildFn<'a> = dyn Fn(&Props) -> Result<ActorRef, ActorError> + Send + Sync + 'a;
type ReplyFn<'a> = dyn Fn(AnyOwnedMessage) -> Result<(), SendError<AnyOwnedMessage>> + Send + Sync + 'a;

/// Execution context passed to actor lifecycle callbacks.
///
/// The context exposes runtime hooks such as child spawning, access to the actor's PID, and helper
/// utilities for replying through an associated [`ActorRef`].
pub struct ActorContext<'a> {
  self_pid:    &'a Pid,
  spawn_child: Option<&'a SpawnChildFn<'a>>,
  reply_fn:    Option<&'a ReplyFn<'a>>,
  reply_to:    Option<&'a ActorRef>,
}

impl<'a> ActorContext<'a> {
  /// Creates a new context for the provided PID.
  #[must_use]
  pub fn new(self_pid: &'a Pid) -> Self {
    Self { self_pid, spawn_child: None, reply_fn: None, reply_to: None }
  }

  /// Configures the child spawning hook.
  #[must_use]
  pub fn with_spawn_child(mut self, spawn_child: &'a SpawnChildFn<'a>) -> Self {
    self.spawn_child = Some(spawn_child);
    self
  }

  /// Configures the reply callback used for ask flows.
  #[must_use]
  pub fn with_reply_fn(mut self, reply_fn: &'a ReplyFn<'a>) -> Self {
    self.reply_fn = Some(reply_fn);
    self
  }

  /// Associates the current message's reply target.
  #[must_use]
  pub fn with_reply_to(mut self, reply_to: Option<&'a ActorRef>) -> Self {
    self.reply_to = reply_to;
    self
  }

  /// Returns the PID assigned to the current actor.
  #[must_use]
  pub const fn self_pid(&self) -> &Pid {
    self.self_pid
  }

  /// Returns the reply target embedded in the current message if present.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&ActorRef> {
    self.reply_to
  }

  /// Attempts to spawn a new child actor using the configured hook.
  ///
  /// When no hook is configured the operation fails with a fatal error, signaling a runtime bug in
  /// the surrounding actor system.
  pub fn spawn_child(&self, props: &Props) -> Result<ActorRef, ActorError> {
    match self.spawn_child {
      | Some(handler) => handler(props),
      | None => Err(ActorError::fatal("actor_context_missing_spawn_child_handler")),
    }
  }

  /// Replies to the current sender using either the explicit reply function or the captured actor
  /// reference.
  pub fn reply(&self, message: AnyOwnedMessage) -> Result<(), SendError<AnyOwnedMessage>> {
    if let Some(handler) = self.reply_fn {
      return handler(message);
    }

    let Some(reply_to) = self.reply_to else {
      return Err(SendError::no_recipient(None, message));
    };

    reply_to.tell(message)
  }
}
