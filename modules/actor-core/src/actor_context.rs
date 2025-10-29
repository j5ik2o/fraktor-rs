//! Actor context scaffolding.

use core::marker::PhantomData;

use crate::{actor_error::ActorError, any_message::AnyMessage, pid::Pid, props::Props};

type SpawnChildFn<'a> = dyn Fn(&Props) -> Result<Pid, ActorError> + 'a;
type ReplyFn<'a> = dyn for<'msg> Fn(AnyMessage<'msg>) -> Result<(), ActorError> + 'a;

/// Execution context provided to actors while processing messages.
///
/// The context exposes runtime hooks such as spawning child actors and replying
/// to the current sender.  The actual behaviour depends on the runtime wiring;
/// when a hook is not configured the helper methods return
/// [`ActorError::unsupported`].
pub struct ActorContext<'a> {
  self_pid: &'a Pid,
  spawn_child: Option<&'a SpawnChildFn<'a>>,
  reply: Option<&'a ReplyFn<'a>>,
  _marker: PhantomData<&'a ()>,
}

impl<'a> ActorContext<'a> {
  /// Creates a new context bound to the specified actor PID.
  #[must_use]
  pub const fn new(self_pid: &'a Pid) -> Self {
    Self { self_pid, spawn_child: None, reply: None, _marker: PhantomData }
  }

  /// Returns the PID of the running actor.
  #[must_use]
  pub const fn self_pid(&self) -> &Pid {
    self.self_pid
  }

  /// Provides the runtime hook that spawns a child actor.
  #[allow(dead_code)]
  pub(crate) fn set_spawn_child_handler(&mut self, handler: &'a SpawnChildFn<'a>) {
    self.spawn_child = Some(handler);
  }

  /// Provides the runtime hook that replies to the current sender.
  #[allow(dead_code)]
  pub(crate) fn set_reply_handler(&mut self, handler: &'a ReplyFn<'a>) {
    self.reply = Some(handler);
  }

  /// Spawns a child actor using the configured runtime hook.
  pub fn spawn_child(&self, props: &Props) -> Result<Pid, ActorError> {
    match self.spawn_child {
      Some(handler) => handler(props),
      None => Err(ActorError::unsupported("spawn_child")),
    }
  }

  /// Replies to the current sender using the configured runtime hook.
  pub fn reply(&self, message: AnyMessage<'_>) -> Result<(), ActorError> {
    match self.reply {
      Some(handler) => handler(message),
      None => Err(ActorError::unsupported("reply")),
    }
  }
}
