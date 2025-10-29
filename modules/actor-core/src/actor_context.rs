//! Actor context scaffolding.

use core::marker::PhantomData;

use crate::{
  actor_error::ActorError, actor_ref::ActorRef, any_message::AnyMessage, pid::Pid, props::Props, system::SystemShared,
};

type ReplyFn<'a> = dyn for<'msg> Fn(AnyMessage<'msg>) -> Result<(), ActorError> + 'a;

/// Execution context provided to actors while processing messages.
///
/// The context exposes runtime hooks such as spawning child actors and replying
/// to the current sender.  When a hook is未配線の場合は [`ActorError::unsupported`]
/// を返す。
pub struct ActorContext<'a> {
  self_pid: &'a Pid,
  system:   Option<SystemShared>,
  reply:    Option<&'a ReplyFn<'a>>,
  _marker:  PhantomData<&'a ()>,
}

impl<'a> ActorContext<'a> {
  /// Creates a new context bound to the specified actor PID.
  #[must_use]
  pub const fn new(self_pid: &'a Pid) -> Self {
    Self { self_pid, system: None, reply: None, _marker: PhantomData }
  }

  /// Returns the PID of the running actor。
  #[must_use]
  pub const fn self_pid(&self) -> &Pid {
    self.self_pid
  }

  /// Provides the runtime/system handle required for `spawn_child`。
  pub(crate) fn set_system_handle(&mut self, system: SystemShared) {
    self.system = Some(system);
  }

  /// Provides the runtime hook that replies to the current sender。
  #[allow(dead_code)]
  pub(crate) fn set_reply_handler(&mut self, handler: &'a ReplyFn<'a>) {
    self.reply = Some(handler);
  }

  /// Spawns a child actor using the configured runtime handle。
  pub fn spawn_child(&self, props: &Props) -> Result<ActorRef, ActorError> {
    let system = self.system.as_ref().ok_or_else(|| ActorError::unsupported("spawn_child"))?;
    let pid = {
      let mut guard = system.lock();
      guard.spawn_actor(system, None, props.clone())?
    };
    Ok(ActorRef::new(pid, system.clone()))
  }

  /// Replies to the current sender using the configured runtime hook。
  pub fn reply(&self, message: AnyMessage<'_>) -> Result<(), ActorError> {
    match self.reply {
      | Some(handler) => handler(message),
      | None => Err(ActorError::unsupported("reply")),
    }
  }
}
