//! Actor reference handle implementation.

#[cfg(test)]
mod tests;

use core::{
  fmt,
  hash::{Hash, Hasher},
  time::Duration,
};

use fraktor_utils_rs::core::sync::SharedAccess;

use crate::core::kernel::{
  actor::{
    Pid,
    actor_path::ActorPath,
    actor_ref::{ActorRefSender, ActorRefSenderShared, NullSender, ask_reply_sender::AskReplySender},
  },
  error::SendError,
  futures::ActorFutureShared,
  messaging::{AnyMessage, AskError, AskResponse, AskResult, system_message::SystemMessage},
  pattern,
  system::state::{SystemStateShared, SystemStateWeak},
};

/// Handle used to communicate with an actor instance.
///
/// Uses a weak reference to the system state to avoid circular references
/// when actor references are stored in event stream subscribers.
pub struct ActorRef {
  pid:    Pid,
  sender: ActorRefSenderShared,
  system: Option<SystemStateWeak>,
}

impl ActorRef {
  /// Creates a new actor reference backed by the provided sender.
  #[must_use]
  pub fn new<T>(pid: Pid, sender: T) -> Self
  where
    T: ActorRefSender + 'static, {
    Self::from_parts(pid, sender, None)
  }

  /// Creates an actor reference backed by the given sender and system state (path-aware).
  #[must_use]
  pub fn with_system<T>(pid: Pid, sender: T, system: &SystemStateShared) -> Self
  where
    T: ActorRefSender + 'static, {
    Self::from_parts(pid, sender, Some(system.downgrade()))
  }

  fn from_parts<T>(pid: Pid, sender: T, system: Option<SystemStateWeak>) -> Self
  where
    T: ActorRefSender + 'static, {
    Self { pid, sender: ActorRefSenderShared::new(sender), system }
  }

  /// Creates an actor reference from an existing shared sender.
  #[must_use]
  pub fn from_shared(pid: Pid, sender: ActorRefSenderShared, system: &SystemStateShared) -> Self {
    Self { pid, sender, system: Some(system.downgrade()) }
  }

  /// Returns the unique process identifier.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the logical path of the actor if the system is still available.
  #[must_use]
  pub fn path(&self) -> Option<ActorPath> {
    self.system.as_ref().and_then(|weak| weak.upgrade()).and_then(|system| system.actor_path(&self.pid))
  }

  /// Returns the canonical actor path including authority and UID when available.
  #[must_use]
  pub fn canonical_path(&self) -> Option<ActorPath> {
    self.system.as_ref().and_then(|weak| weak.upgrade()).and_then(|system| system.canonical_actor_path(&self.pid))
  }

  /// Returns the underlying system state if available.
  #[must_use]
  pub(crate) fn system_state(&self) -> Option<SystemStateShared> {
    self.system.as_ref().and_then(|weak| weak.upgrade())
  }

  /// Sends a message to the referenced actor (fire-and-forget).
  ///
  /// Failures are recorded via the dead-letter / observation path but never
  /// surfaced to the caller. This matches Pekko's at-most-once `tell` semantics.
  #[cfg(not(fraktor_disable_tell))]
  pub fn tell(&mut self, message: AnyMessage) {
    // 公開 API としての fire-and-forget 契約を維持する。
    if self.try_tell(message).is_err() {}
  }

  /// Sends a message through the underlying sender and preserves synchronous
  /// delivery failures.
  ///
  /// Use this when the caller must observe enqueue failure explicitly.
  /// [`tell`](Self::tell) remains the fire-and-forget variant.
  ///
  /// On failure the error is also recorded via the system's observation path
  /// when the reference is path-aware.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the underlying sender rejects the message.
  pub fn try_tell(&mut self, message: AnyMessage) -> Result<(), SendError> {
    let result = self.sender.send(message);
    if let Err(error) = &result
      && let Some(system) = self.system.as_ref().and_then(|weak| weak.upgrade())
    {
      system.record_send_error(Some(self.pid), error);
    }
    result
  }

  /// Sends `PoisonPill` to the referenced actor via the user message channel.
  pub fn poison_pill(&mut self) {
    if self.try_poison_pill().is_err() {}
  }

  /// Sends `PoisonPill` to the referenced actor via the user message channel.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the underlying mailbox rejects the message.
  pub fn try_poison_pill(&mut self) -> Result<(), SendError> {
    self.try_tell(AnyMessage::new(SystemMessage::PoisonPill))
  }

  /// Sends `Kill` to the referenced actor via the user message channel.
  pub fn kill(&mut self) {
    if self.try_kill().is_err() {}
  }

  /// Sends `Kill` to the referenced actor via the user message channel.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the underlying mailbox rejects the message.
  pub fn try_kill(&mut self) -> Result<(), SendError> {
    self.try_tell(AnyMessage::new(SystemMessage::Kill))
  }

  /// Sends a request and obtains a future that resolves with the reply.
  ///
  /// The returned future resolves with `Ok(message)` on success, or
  /// `Err(AskError)` when the request times out or the reply path fails.
  #[must_use]
  pub fn ask(&mut self, message: AnyMessage) -> AskResponse {
    let future = ActorFutureShared::<AskResult>::new();
    let reply_sender = AskReplySender::new(future.clone());
    let reply_ref = ActorRef::new(self.pid, reply_sender);
    let envelope = message.with_sender(reply_ref.clone());
    if let Err(error) = self.try_tell(envelope) {
      let waker = future.with_write(|inner| inner.complete(Err(AskError::from(&error))));
      if let Some(waker) = waker {
        waker.wake();
      }
      return AskResponse::new(reply_ref, future);
    }
    if let Some(system) = self.system.as_ref().and_then(|weak| weak.upgrade()) {
      system.register_ask_future(future.clone());
    }
    AskResponse::new(reply_ref, future)
  }

  /// Sends a request and arranges timeout completion on the returned ask future.
  #[must_use]
  pub fn ask_with_timeout(&mut self, message: AnyMessage, timeout: Duration) -> AskResponse {
    pattern::ask_with_timeout(self, message, timeout)
  }

  /// Creates a placeholder reference that rejects all messages.
  #[must_use]
  pub fn null() -> Self {
    Self { pid: Pid::new(0, 0), sender: ActorRefSenderShared::new(NullSender), system: None }
  }

  /// Returns a sentinel reference indicating "no sender".
  ///
  /// This mirrors Pekko's `Actor.noSender` and is equivalent to [`ActorRef::null`].
  #[must_use]
  pub fn no_sender() -> Self {
    Self::null()
  }
}

impl Clone for ActorRef {
  fn clone(&self) -> Self {
    Self { pid: self.pid, sender: self.sender.clone(), system: self.system.clone() }
  }
}

// SAFETY: `ActorRef` holds `ArcShared` handles to trait objects that are required to be both
// `Send` and `Sync`. Cloning or dropping the reference does not violate thread-safety guarantees.
unsafe impl Send for ActorRef {}

unsafe impl Sync for ActorRef {}

impl fmt::Debug for ActorRef {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ActorRef").field("pid", &self.pid).finish()
  }
}

impl PartialEq for ActorRef {
  fn eq(&self, other: &Self) -> bool {
    self.pid == other.pid
  }
}

impl Eq for ActorRef {}

impl Hash for ActorRef {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.pid.hash(state);
  }
}
