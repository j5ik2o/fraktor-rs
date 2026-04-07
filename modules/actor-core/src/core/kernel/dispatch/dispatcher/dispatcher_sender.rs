//! `ActorRefSender` implementation backed by `MessageDispatcherShared`.
//!
//! `DispatcherSender` is constructed in `ActorCell::create` whenever the
//! actor system has a dispatcher configurator registered for the resolved
//! dispatcher id. It routes every `ActorRef::tell` through the dispatcher's
//! own `dispatch` hook (default: enqueue into the receiver mailbox;
//! `BalancingDispatcher`: enqueue into the shared team queue) so the
//! dispatcher decides where the envelope lands. Bypassing the trait hook and
//! enqueuing directly on `receiver.mailbox` would break `BalancingDispatcher`
//! load balancing.
//!
//! # Two-phase send (re-entrancy contract)
//!
//! The send is split into two phases:
//!
//! 1. [`MessageDispatcherShared::dispatch_enqueue`] runs **inside**
//!    `ActorRefSenderShared`'s per-actor sender lock. It briefly acquires
//!    the dispatcher write lock, calls the trait `dispatch` hook (which
//!    enqueues the envelope into the appropriate queue), releases the
//!    dispatcher lock, and returns the candidate mailbox list.
//! 2. The returned [`SendOutcome::Schedule`] closure runs **after**
//!    `ActorRefSenderShared` has released the per-actor sender lock and
//!    invokes [`MessageDispatcherShared::register_user_candidates`], which
//!    in turn calls `register_for_execution` for each candidate. With an
//!    inline executor, `register_for_execution` synchronously runs
//!    `mailbox.run(...)`, so user-supplied message handlers execute on the
//!    calling thread without holding the per-actor sender lock. This is
//!    what lets a handler legally re-enter the same actor's `tell` (for
//!    example via `ctx.ask(...)` + `pipe_to_self`) without deadlocking on
//!    the sender mutex.
//!
//! The sender holds only the receiver mailbox. The owning [`ActorCell`] is
//! resolved via `Mailbox::actor()` on each `send`, which avoids an
//! `ActorCell -> sender -> ActorCell` ownership cycle.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::ArcShared;

use super::message_dispatcher_shared::MessageDispatcherShared;
use crate::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  dispatch::mailbox::{Envelope, Mailbox},
};

/// Sender that routes user messages through the dispatcher tree.
pub struct DispatcherSender {
  dispatcher: MessageDispatcherShared,
  mailbox:    ArcShared<Mailbox>,
}

impl DispatcherSender {
  /// Builds a new sender bound to `dispatcher` and `mailbox`.
  #[must_use]
  pub const fn new(dispatcher: MessageDispatcherShared, mailbox: ArcShared<Mailbox>) -> Self {
    Self { dispatcher, mailbox }
  }
}

impl ActorRefSender for DispatcherSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let envelope = Envelope::new(message);
    // Resolve the owning ActorCell through the mailbox's installed weak
    // reference. `ActorCell::create` installs the weak handle on the mailbox
    // before the cell becomes observable externally, so this upgrade only
    // fails after the cell has been dropped, at which point reporting
    // `closed` is the correct answer.
    let Some(cell) = self.mailbox.actor().and_then(|weak| weak.upgrade()) else {
      return Err(SendError::closed(envelope.into_payload()));
    };
    // Phase 1 (inside the per-actor sender lock): enqueue via the trait
    // dispatch hook so `BalancingDispatcher` can route into its shared team
    // queue. Returns the candidate mailbox list without scheduling.
    let candidates = self.dispatcher.dispatch_enqueue(&cell, envelope)?;
    // Phase 2 (after the sender lock is released by `ActorRefSenderShared`):
    // schedule the returned candidates. With an inline executor this runs
    // `mailbox.run(...)` synchronously, so it must execute outside the
    // sender lock to keep nested same-actor `tell` calls deadlock-free.
    let dispatcher = self.dispatcher.clone();
    let schedule = move || {
      dispatcher.register_user_candidates(&candidates);
    };
    Ok(SendOutcome::Schedule(Box::new(schedule)))
  }
}
