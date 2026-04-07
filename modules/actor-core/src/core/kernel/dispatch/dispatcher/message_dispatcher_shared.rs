//! AShared wrapper around `Box<dyn MessageDispatcher>`.
//!
//! `MessageDispatcherShared` is the only legal way to share a concrete
//! [`MessageDispatcher`] across actor cells, dispatcher registries, and
//! callers. It owns the `RuntimeMutex` so trait commands always run under
//! `&mut self`, and it exposes the lifecycle and dispatch orchestration that
//! must execute outside the lock (executor submission, delayed shutdown
//! registration, etc.).

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::{
  executor_shared::ExecutorShared, message_dispatcher::MessageDispatcher, shutdown_schedule::ShutdownSchedule,
};
use crate::core::kernel::{
  actor::{ActorCell, error::SendError, messaging::system_message::SystemMessage, spawn::SpawnError},
  dispatch::mailbox::{Envelope, Mailbox, ScheduleHints},
};

/// Shared wrapper providing thread-safe orchestration around a `MessageDispatcher`.
pub struct MessageDispatcherShared {
  inner: ArcShared<RuntimeMutex<Box<dyn MessageDispatcher>>>,
}

impl Clone for MessageDispatcherShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl MessageDispatcherShared {
  /// Wraps the supplied dispatcher in a shared handle.
  #[must_use]
  pub fn new<D: MessageDispatcher + 'static>(dispatcher: D) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(Box::new(dispatcher) as Box<dyn MessageDispatcher>)) }
  }

  /// Returns the dispatcher identifier.
  #[must_use]
  pub fn id(&self) -> alloc::string::String {
    use alloc::string::ToString;
    self.with_read(|inner| inner.id().to_string())
  }

  /// Returns the dispatcher throughput.
  #[must_use]
  pub fn throughput(&self) -> core::num::NonZeroUsize {
    self.with_read(|inner| inner.throughput())
  }

  /// Returns the dispatcher throughput deadline.
  #[must_use]
  pub fn throughput_deadline(&self) -> Option<core::time::Duration> {
    self.with_read(|inner| inner.throughput_deadline())
  }

  /// Returns the dispatcher shutdown timeout.
  #[must_use]
  pub fn shutdown_timeout(&self) -> core::time::Duration {
    self.with_read(|inner| inner.shutdown_timeout())
  }

  /// Returns the current inhabitants count.
  #[must_use]
  pub fn inhabitants(&self) -> i64 {
    self.with_read(|inner| inner.inhabitants())
  }

  /// Returns a clone of the underlying executor shared handle.
  #[must_use]
  pub fn executor(&self) -> ExecutorShared {
    self.with_read(|inner| inner.executor())
  }

  /// Returns a pre-built shared mailbox if the inner dispatcher requires one.
  ///
  /// Delegates to
  /// [`MessageDispatcher::try_create_shared_mailbox`](super::MessageDispatcher::try_create_shared_mailbox).
  /// Returns `None` for dispatchers that want `ActorCell::create` to build a
  /// per-actor mailbox from `MailboxConfig`; returns `Some` for dispatchers
  /// like `BalancingDispatcher` whose team members must drain a shared queue.
  #[must_use]
  pub fn try_create_shared_mailbox(&self) -> Option<ArcShared<Mailbox>> {
    self.with_read(|inner| inner.try_create_shared_mailbox())
  }

  /// Attaches `actor` to the dispatcher and arranges initial scheduling.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the dispatcher rejects the attach request
  /// (for example, `PinnedDispatcher::DispatcherAlreadyOwned`).
  pub fn attach(&self, actor: &ArcShared<ActorCell>) -> Result<(), SpawnError> {
    let mailbox = self.with_write(|inner| -> Result<ArcShared<Mailbox>, SpawnError> {
      inner.register_actor(actor)?;
      // The new dispatcher path takes ownership of mailbox creation in
      // Phase 11. During the parallel period the legacy create-cell path has
      // already installed `actor.mailbox()`, so we simply observe the
      // existing mailbox here.
      Ok(actor.mailbox())
    })?;
    // The boolean return is best-effort here: if the mailbox was already
    // scheduled by another thread we don't need to do anything.
    if !self.register_for_execution(&mailbox, false, true) {
      tracing::trace!(
        target: "fraktor::dispatcher",
        "attach observed mailbox already scheduled or executor unavailable"
      );
    }
    Ok(())
  }

  /// Detaches `actor` from the dispatcher.
  ///
  /// Transitions the actor's mailbox into the closed terminal state and runs
  /// `clean_up` so any remaining envelopes are routed to dead letters (the
  /// `MailboxCleanupPolicy::LeaveSharedQueue` variant used by
  /// `BalancingDispatcher` skips the drain). Returns the post-detach
  /// [`ShutdownSchedule`] so callers can decide whether to register a delayed
  /// dispatcher shutdown.
  #[must_use]
  pub fn detach(&self, actor: &ArcShared<ActorCell>) -> ShutdownSchedule {
    actor.mailbox().become_closed_and_clean_up();
    self.with_write(|inner| {
      inner.unregister_actor(actor);
      inner.core_mut().schedule_shutdown_if_sensible()
    })
  }

  /// Dispatches a user envelope through the inner dispatcher.
  ///
  /// Acquires the dispatcher write lock briefly to call the trait `dispatch`
  /// hook (which enqueues the envelope and returns the candidate mailbox
  /// list), then attempts to schedule the candidates **after** the lock is
  /// released. The schedule step is not deferred, so this method blocks for
  /// the entire `register_for_execution` chain (including
  /// `executor.execute(...)`).
  ///
  /// **Re-entrancy warning**: with an inline executor, the `executor.execute`
  /// call inside `register_for_execution` synchronously runs `mailbox.run(...)`
  /// on the calling thread. If the caller is itself holding an unrelated
  /// lock (for example
  /// [`ActorRefSenderShared`](crate::core::kernel::actor::actor_ref::ActorRefSenderShared)
  /// keeps the per-actor sender mutex while it invokes `send`), the nested
  /// `mailbox.run` may try to re-enter the same lock and deadlock. The
  /// `ActorRef::tell` send path therefore avoids this convenience method and
  /// uses [`dispatch_enqueue`](Self::dispatch_enqueue) +
  /// [`register_user_candidates`](Self::register_user_candidates) so the
  /// scheduling chain runs after the per-actor sender lock is released.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the inner dispatcher rejects the envelope.
  pub fn dispatch(&self, receiver: &ArcShared<ActorCell>, envelope: Envelope) -> Result<(), SendError> {
    let candidates = self.dispatch_enqueue(receiver, envelope)?;
    self.register_user_candidates(&candidates);
    Ok(())
  }

  /// Enqueues `envelope` for `receiver` via the trait `dispatch` hook and
  /// returns the candidate mailbox list **without** scheduling them.
  ///
  /// This is the lock-safe primitive used by `DispatcherSender::send` so that
  /// the actual `register_for_execution` chain (which may run
  /// `mailbox.run(...)` synchronously under an inline executor) happens
  /// **after** the per-actor `ActorRefSenderShared` lock is released. Doing
  /// the schedule step inside the sender lock would let a nested
  /// re-entrant `tell` from the message handler deadlock against the same
  /// per-actor sender mutex.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the inner dispatcher rejects the envelope.
  pub fn dispatch_enqueue(
    &self,
    receiver: &ArcShared<ActorCell>,
    envelope: Envelope,
  ) -> Result<alloc::vec::Vec<ArcShared<Mailbox>>, SendError> {
    self.with_write(|inner| inner.dispatch(receiver, envelope))
  }

  /// Schedules a user-message candidate list returned by
  /// [`dispatch_enqueue`](Self::dispatch_enqueue).
  ///
  /// Iterates the candidates in priority order and stops at the first
  /// mailbox that successfully transitioned from idle to scheduled. Intended
  /// to be called outside any per-actor sender lock to keep the inline
  /// executor re-entrancy contract intact.
  pub fn register_user_candidates(&self, candidates: &[ArcShared<Mailbox>]) {
    self.try_register_candidates(candidates, true, false);
  }

  /// Dispatches a system message through the inner dispatcher.
  ///
  /// Like [`dispatch`](Self::dispatch) this acquires the write lock briefly
  /// and then schedules the returned candidates inline. System message
  /// senders do not currently route through `ActorRefSenderShared`, so the
  /// inline schedule path is safe here.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the inner dispatcher rejects the message.
  pub fn system_dispatch(&self, receiver: &ArcShared<ActorCell>, message: SystemMessage) -> Result<(), SendError> {
    let candidates = self.with_write(|inner| inner.system_dispatch(receiver, message))?;
    self.try_register_candidates(&candidates, false, true);
    Ok(())
  }

  /// Suspends the actor on the inner dispatcher.
  pub fn suspend(&self, actor: &ArcShared<ActorCell>) {
    self.with_write(|inner| inner.suspend(actor));
  }

  /// Resumes the actor on the inner dispatcher.
  pub fn resume(&self, actor: &ArcShared<ActorCell>) {
    self.with_write(|inner| inner.resume(actor));
  }

  /// Shuts the inner dispatcher down.
  pub fn shutdown(&self) {
    self.with_write(|inner| inner.shutdown());
  }

  /// Attempts to schedule the given mailbox for execution on the dispatcher's executor.
  ///
  /// Returns `true` if the mailbox transitioned from idle to scheduled and was
  /// successfully submitted to the executor.
  #[must_use]
  pub fn register_for_execution(
    &self,
    mailbox: &ArcShared<Mailbox>,
    has_message_hint: bool,
    has_system_hint: bool,
  ) -> bool {
    let hints = ScheduleHints {
      has_system_messages: has_system_hint,
      has_user_messages:   has_message_hint,
      backpressure_active: false,
    };
    if !mailbox.request_schedule(hints) {
      return false;
    }

    let throughput = self.throughput();
    let throughput_deadline = self.throughput_deadline();
    let executor = self.executor();
    let mbox_clone = mailbox.clone();
    // Capture a clone of the dispatcher so the post-drain reschedule path
    // can re-call `register_for_execution` if more work arrived during the
    // drain. Without this, `Mailbox::run` would consume the
    // `need_reschedule` flag silently and the late-arriving messages would
    // sit in the queue until the next external `tell()` (which may never
    // come — e.g. when the producer has already submitted the entire batch
    // before the receiver finished its first throughput-limited drain).
    let dispatcher = self.clone();
    let result = executor.execute(Box::new(move || {
      let needs_reschedule = mbox_clone.run(throughput, throughput_deadline);
      if needs_reschedule {
        // Re-arm the schedule. We don't know whether the pending work is a
        // user message or a system message at this point, so signal both
        // hint flags conservatively — `request_schedule` will only refuse
        // when the mailbox is closed or already scheduled by another
        // thread, both of which are safe outcomes.
        let _ = dispatcher.register_for_execution(&mbox_clone, true, true);
      }
    }));

    match result {
      | Ok(()) => true,
      | Err(_error) => {
        // Roll back the CAS so the mailbox can be retried later. The bool
        // returned by `set_idle` indicates whether an immediate reschedule is
        // required, but we're already on a failure path and the executor is
        // unavailable, so observing the value would not help.
        if mailbox.set_idle() {
          tracing::debug!(
            target: "fraktor::dispatcher",
            "register_for_execution rolled back after submit failure with pending reschedule"
          );
        }
        false
      },
    }
  }

  fn try_register_candidates(&self, candidates: &[ArcShared<Mailbox>], message_hint: bool, system_hint: bool) {
    for mailbox in candidates {
      if self.register_for_execution(mailbox, message_hint, system_hint) {
        break;
      }
    }
  }
}

impl SharedAccess<Box<dyn MessageDispatcher>> for MessageDispatcherShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageDispatcher>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageDispatcher>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
