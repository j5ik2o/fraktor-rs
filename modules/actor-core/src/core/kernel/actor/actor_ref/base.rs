//! Actor reference handle implementation.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  hash::{Hash, Hasher},
  time::Duration,
};

use fraktor_utils_core_rs::core::sync::SharedAccess;
use portable_atomic::{AtomicU64, Ordering};

use crate::core::kernel::{
  actor::{
    Pid,
    actor_path::ActorPath,
    actor_ref::{
      ActorRefSender, ActorRefSenderShared, ActorRefSenderSharedFactory, NullSender, ask_reply_sender::AskReplySender,
    },
    error::SendError,
    messaging::{AnyMessage, AskError, AskResponse, AskResult, system_message::SystemMessage},
  },
  pattern,
  system::{
    shared_factory::BuiltinSpinSharedFactory,
    state::{SystemStateShared, SystemStateWeak},
  },
  util::futures::{ActorFuture, ActorFutureShared, ActorFutureSharedFactory},
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

// Fallback reply pid generator used only when no system state is attached.
// Start away from runtime-allocated low pids and reserved facade pids.
static ASK_REPLY_FALLBACK_PID: AtomicU64 = AtomicU64::new(u64::MAX / 2);

impl ActorRef {
  fn create_builtin_actor_ref_sender_shared(sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderSharedFactory::create_actor_ref_sender_shared(&BuiltinSpinSharedFactory::new(), sender)
  }

  fn create_builtin_actor_future_shared(future: ActorFuture<AskResult>) -> ActorFutureShared<AskResult> {
    ActorFutureSharedFactory::create_actor_future_shared(&BuiltinSpinSharedFactory::new(), future)
  }

  fn complete_ask_future_with_error(future: &ActorFutureShared<AskResult>, error: &SendError) {
    let waker = future.with_write(|inner| inner.complete(Err(AskError::from(error))));
    if let Some(waker) = waker {
      waker.wake();
    }
  }

  fn register_ask_future_if_available(system: Option<SystemStateShared>, future: &ActorFutureShared<AskResult>) {
    if let Some(system) = system {
      system.register_ask_future(future.clone());
    }
  }

  fn build_ask_reply_ref(
    system: Option<&SystemStateShared>,
    path_aware_reply: bool,
    reply_sender: AskReplySender,
  ) -> Self {
    let pid = Self::next_ask_reply_pid(system);
    if let Some(system) = system {
      let sender = system.actor_ref_sender_shared_factory().create_actor_ref_sender_shared(Box::new(reply_sender));
      return if path_aware_reply { Self::from_shared(pid, sender, system) } else { Self::new(pid, sender) };
    }
    let sender = Self::create_builtin_actor_ref_sender_shared(Box::new(reply_sender));
    Self::new(pid, sender)
  }

  fn next_ask_reply_pid(system: Option<&SystemStateShared>) -> Pid {
    if let Some(system) = system {
      return system.allocate_pid();
    }

    let raw = ASK_REPLY_FALLBACK_PID.fetch_add(1, Ordering::Relaxed);
    Pid::new(raw, 0)
  }

  /// Creates a new actor reference backed by an existing shared sender.
  #[must_use]
  pub const fn new(pid: Pid, sender: ActorRefSenderShared) -> Self {
    Self { pid, sender, system: None }
  }

  /// Creates a new actor reference backed by the built-in sender lock.
  ///
  /// This helper is public only for tests and `test-support` consumers.
  #[must_use]
  #[cfg(any(test, feature = "test-support"))]
  pub fn new_with_builtin_lock<T>(pid: Pid, sender: T) -> Self
  where
    T: ActorRefSender + 'static, {
    Self::new_with_builtin_lock_impl(pid, sender)
  }

  #[cfg(any(test, feature = "test-support"))]
  fn new_with_builtin_lock_impl<T>(pid: Pid, sender: T) -> Self
  where
    T: ActorRefSender + 'static, {
    let sender = Self::create_builtin_actor_ref_sender_shared(Box::new(sender));
    Self { pid, sender, system: None }
  }

  /// Creates an actor reference backed by the given sender and system state (path-aware).
  #[must_use]
  pub fn with_system<T>(pid: Pid, sender: T, system: &SystemStateShared) -> Self
  where
    T: ActorRefSender + 'static, {
    let sender = system.actor_ref_sender_shared_factory().create_actor_ref_sender_shared(Box::new(sender));
    Self::from_shared(pid, sender, system)
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

  /// Sends a request built from a reply target and obtains the associated ask response.
  ///
  /// `path_aware_reply` controls whether the reply actor ref keeps the originating
  /// system attached. Reply refs always use a distinct PID from the target actor
  /// so they do not collide in equality, hashing, or path resolution.
  #[must_use]
  pub(crate) fn ask_with_factory<F>(&mut self, path_aware_reply: bool, build: F) -> AskResponse
  where
    F: FnOnce(ActorRef) -> AnyMessage, {
    let system = self.system_state();
    let future = system
      .as_ref()
      .map(|state| state.actor_future_shared_factory().create_actor_future_shared(ActorFuture::new()))
      .unwrap_or_else(|| Self::create_builtin_actor_future_shared(ActorFuture::new()));
    let reply_sender = AskReplySender::new(future.clone());
    let reply_ref = Self::build_ask_reply_ref(system.as_ref(), path_aware_reply, reply_sender);
    let message = build(reply_ref.clone());

    if let Err(error) = self.try_tell(message) {
      Self::complete_ask_future_with_error(&future, &error);
      return AskResponse::new(reply_ref, future);
    }

    Self::register_ask_future_if_available(system, &future);
    AskResponse::new(reply_ref, future)
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
    self.ask_with_factory(false, |reply_ref| message.with_sender(reply_ref))
  }

  /// Sends a request and arranges timeout completion on the returned ask future.
  #[must_use]
  pub fn ask_with_timeout(&mut self, message: AnyMessage, timeout: Duration) -> AskResponse {
    pattern::ask_with_timeout(self, message, timeout)
  }

  /// Creates a placeholder reference that rejects all messages.
  #[must_use]
  pub fn null() -> Self {
    let sender = Self::create_builtin_actor_ref_sender_shared(Box::new(NullSender));
    Self::new(Pid::new(0, 0), sender)
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

impl Debug for ActorRef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
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
