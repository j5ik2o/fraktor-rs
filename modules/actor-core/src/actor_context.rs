//! Actor context scaffolding.

use core::marker::PhantomData;

use crate::{
  actor_error::ActorError, actor_ref::ActorRef, any_owned_message::AnyOwnedMessage, child_ref::ChildRef, pid::Pid,
  props::Props, system::SystemShared,
};

/// Execution context provided to actors while processing messages.
///
/// The context exposes runtime hooks such as spawning child actors and replying
/// to the current sender. When a hook is unavailable the runtime returns
/// [`ActorError::unsupported`].
pub struct ActorContext<'a> {
  self_pid: &'a Pid,
  system:   Option<SystemShared>,
  reply_to: Option<ActorRef>,
  _marker:  PhantomData<&'a ()>,
}

impl<'a> ActorContext<'a> {
  /// Creates a new context bound to the specified actor PID.
  #[must_use]
  pub const fn new(self_pid: &'a Pid) -> Self {
    Self { self_pid, system: None, reply_to: None, _marker: PhantomData }
  }

  /// Returns the PID of the running actor.
  #[must_use]
  pub const fn self_pid(&self) -> &Pid {
    self.self_pid
  }

  /// Provides the runtime/system handle required for `spawn_child`.
  pub(crate) fn set_system_handle(&mut self, system: SystemShared) {
    self.system = Some(system);
  }

  /// Records the reply target for the current message.
  pub(crate) fn set_reply_target(&mut self, reply_to: Option<ActorRef>) {
    self.reply_to = reply_to;
  }

  /// Spawns a child actor using the configured runtime handle.
  pub fn spawn_child(&self, props: &Props) -> Result<ChildRef, ActorError> {
    let system = self.system.as_ref().ok_or_else(|| ActorError::unsupported("spawn_child"))?;
    let pid = {
      let mut guard = system.lock();
      guard.spawn_actor(system, Some(*self.self_pid), None, *props)?
    };
    Ok(ChildRef::new(ActorRef::new(pid, system.clone())))
  }

  /// Returns the reply target associated with the current message.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorRef> {
    self.reply_to.as_ref()
  }

  /// Replies to the current sender using the stored reply target.
  pub fn reply(&self, message: AnyOwnedMessage) -> Result<(), ActorError> {
    let Some(target) = self.reply_to() else {
      return Err(ActorError::unsupported("reply"));
    };
    target.tell(message).map_err(|_| ActorError::recoverable("reply_failed"))
  }
}
