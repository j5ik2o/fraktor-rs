//! Trait shape of a `MessageDispatcher` (Pekko-aligned).
//!
//! `MessageDispatcher` is the dispatcher-side query/hook surface. The
//! orchestration side (lock-acquisition, mailbox installation, executor
//! submission, delayed shutdown registration) lives on
//! [`MessageDispatcherShared`](super::MessageDispatcherShared) so that the
//! `RuntimeMutex` is acquired only briefly and never held while submitting
//! work to an executor.
//!
//! # CQS contract
//!
//! - **Queries** read state and take `&self`.
//! - **Commands / hooks** mutate state (or any of the actors registered with the dispatcher) and
//!   take `&mut self`.
//! - `create_mailbox` is a factory and stays `&self`.
//!
//! # Hook conventions
//!
//! - `register_actor` / `unregister_actor` / `dispatch` / `system_dispatch` / `create_mailbox` are
//!   overridable hooks. The default `register_actor` / `unregister_actor` impls just delegate to
//!   [`DispatcherCore`] inhabitants bookkeeping. `PinnedDispatcher` overrides them to enforce 1:1
//!   ownership.
//! - `dispatch` / `system_dispatch` enqueue the message into a mailbox queue and return the
//!   **candidate mailbox list** that the shared wrapper should try to schedule for execution. The
//!   candidate list is ordered by priority; the wrapper stops at the first mailbox that
//!   successfully transitions from idle to scheduled. `BalancingDispatcher` returns multiple
//!   candidates so a busy receiver can fall back to a sibling.
//! - `register_for_execution` is **intentionally absent** from this trait. The shared wrapper holds
//!   the only `register_for_execution` path so that trait hooks cannot accidentally re-enter the
//!   lock.
//! - `execute_task` is also absent: it is out of scope for the `dispatcher-pekko-1n-redesign`
//!   change (YAGNI) and will be added when a concrete caller appears.
//!
//! # Actor reference
//!
//! Hook arguments take `&ArcShared<ActorCell>` so that
//! `BalancingDispatcher::create_mailbox` can call `ArcShared::downgrade` to
//! obtain the `WeakShared<ActorCell>` required by `Mailbox::new_sharing`.

use alloc::{boxed::Box, vec};

use fraktor_utils_rs::core::sync::ArcShared;

use super::{dispatcher_core::DispatcherCore, executor_shared::ExecutorShared};
use crate::core::kernel::{
  actor::{ActorCell, error::SendError, messaging::system_message::SystemMessage, spawn::SpawnError},
  dispatch::mailbox::{Envelope, Mailbox, MailboxType},
};

/// Hook/query surface of a dispatcher.
///
/// A `MessageDispatcher` is always shared via
/// [`MessageDispatcherShared`](super::MessageDispatcherShared); concrete
/// implementations should never expose their `&mut self` methods directly.
pub trait MessageDispatcher: Send + Sync {
  // ---- core accessor (mandatory) -------------------------------------------------

  /// Returns a reference to the dispatcher's [`DispatcherCore`].
  fn core(&self) -> &DispatcherCore;

  /// Returns a mutable reference to the dispatcher's [`DispatcherCore`].
  fn core_mut(&mut self) -> &mut DispatcherCore;

  // ---- queries (delegated to core in default impls) -----------------------------

  /// Returns the dispatcher identifier.
  fn id(&self) -> &str {
    self.core().id()
  }

  /// Returns the configured throughput.
  fn throughput(&self) -> core::num::NonZeroUsize {
    self.core().throughput()
  }

  /// Returns the configured throughput deadline.
  fn throughput_deadline(&self) -> Option<core::time::Duration> {
    self.core().throughput_deadline()
  }

  /// Returns the configured shutdown timeout.
  fn shutdown_timeout(&self) -> core::time::Duration {
    self.core().shutdown_timeout()
  }

  /// Returns the current inhabitants count.
  fn inhabitants(&self) -> i64 {
    self.core().inhabitants()
  }

  /// Returns a clone of the underlying `ExecutorShared`.
  fn executor(&self) -> ExecutorShared {
    self.core().executor().clone()
  }

  // ---- factory ------------------------------------------------------------------

  /// Builds a new mailbox for `actor`.
  ///
  /// The default implementation returns a fresh per-actor mailbox using the
  /// queue produced by `mailbox_type`. `BalancingDispatcher` overrides this
  /// to construct a sharing mailbox over the dispatcher's shared queue.
  ///
  /// **Operational discipline**: callers must invoke this through
  /// [`MessageDispatcherShared::attach`](super::MessageDispatcherShared::attach)
  /// so that inhabitants bookkeeping happens in the same critical section.
  fn create_mailbox(&self, actor: &ArcShared<ActorCell>, mailbox_type: &dyn MailboxType) -> ArcShared<Mailbox> {
    let _ = (actor, mailbox_type);
    // Default impl is provided by concrete dispatchers; trait callers must
    // either supply an override or rely on `MessageDispatcherShared::attach`
    // which goes through the concrete implementation directly.
    panic!("MessageDispatcher::create_mailbox must be overridden by concrete dispatchers")
  }

  // ---- overridable hooks --------------------------------------------------------

  /// Registers `actor` with the dispatcher.
  ///
  /// The default implementation increments the inhabitants counter via
  /// [`DispatcherCore::mark_attach`].
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the dispatcher refuses the actor (for
  /// example, `PinnedDispatcher` rejects a second owner with
  /// `SpawnError::DispatcherAlreadyOwned`).
  fn register_actor(&mut self, actor: &ArcShared<ActorCell>) -> Result<(), SpawnError> {
    let _ = actor;
    self.core_mut().mark_attach();
    Ok(())
  }

  /// Unregisters `actor` from the dispatcher.
  ///
  /// The default implementation decrements the inhabitants counter via
  /// [`DispatcherCore::mark_detach`].
  fn unregister_actor(&mut self, actor: &ArcShared<ActorCell>) {
    let _ = actor;
    self.core_mut().mark_detach();
  }

  /// Enqueues a user message for `receiver` and returns the candidate mailbox list.
  ///
  /// The default implementation enqueues the envelope into
  /// `receiver.mailbox()` and returns a single-entry list. Concrete
  /// dispatchers may override this to enqueue into a shared queue and return
  /// additional fallback candidates.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the underlying queue rejects the envelope
  /// (full, closed, etc.).
  fn dispatch(
    &mut self,
    receiver: &ArcShared<ActorCell>,
    envelope: Envelope,
  ) -> Result<alloc::vec::Vec<ArcShared<Mailbox>>, SendError> {
    let mailbox = receiver.mailbox();
    // Backpressure: the default impl drops the pending future and leaves the
    // envelope in-flight. Hot-path callers that care about async backpressure
    // should go through `NewDispatcherSender::send`, which polls the future
    // via `DispatcherWaker` before returning.
    let _ = mailbox.enqueue_envelope(envelope)?;
    Ok(vec![mailbox])
  }

  /// Enqueues a system message for `receiver` and returns the candidate mailbox list.
  ///
  /// The default implementation enqueues the system message into
  /// `receiver.mailbox()` and returns a single-entry list.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the system queue rejects the message.
  fn system_dispatch(
    &mut self,
    receiver: &ArcShared<ActorCell>,
    message: SystemMessage,
  ) -> Result<alloc::vec::Vec<ArcShared<Mailbox>>, SendError> {
    let mailbox = receiver.mailbox();
    mailbox.enqueue_system(message)?;
    Ok(vec![mailbox])
  }

  /// Suspends `actor` from receiving further user messages.
  fn suspend(&mut self, actor: &ArcShared<ActorCell>) {
    let _ = actor;
  }

  /// Resumes `actor` after a previous [`suspend`](Self::suspend) call.
  fn resume(&mut self, actor: &ArcShared<ActorCell>) {
    let _ = actor;
  }

  /// Shuts the dispatcher down.
  ///
  /// The default implementation delegates to [`DispatcherCore::shutdown`].
  fn shutdown(&mut self) {
    self.core_mut().shutdown();
  }
}

// Helper marker so trait objects compile cleanly.
#[allow(dead_code)]
fn _assert_object_safe(_: &dyn MessageDispatcher) {}

#[allow(dead_code)]
fn _assert_box_object_safe(_: Box<dyn MessageDispatcher>) {}
