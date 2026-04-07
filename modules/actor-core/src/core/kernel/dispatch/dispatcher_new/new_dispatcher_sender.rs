//! `ActorRefSender` implementation backed by the new `MessageDispatcherShared`.
//!
//! `NewDispatcherSender` is constructed in `ActorCell::create` whenever the
//! actor system has a `dispatcher_new` configurator registered for the
//! resolved dispatcher id. The sender enqueues envelopes directly into the
//! receiver mailbox and asks the new dispatcher to schedule it.

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
  dispatch::mailbox::{EnqueueOutcome, Envelope, Mailbox},
};

/// Sender that routes user messages through the new dispatcher tree.
pub struct NewDispatcherSender {
  dispatcher: MessageDispatcherShared,
  mailbox:    ArcShared<Mailbox>,
}

impl NewDispatcherSender {
  /// Builds a new sender bound to `dispatcher` and `mailbox`.
  #[must_use]
  pub const fn new(dispatcher: MessageDispatcherShared, mailbox: ArcShared<Mailbox>) -> Self {
    Self { dispatcher, mailbox }
  }
}

impl ActorRefSender for NewDispatcherSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let envelope = Envelope::new(message);
    self.mailbox.enqueue_envelope(envelope)?;
    let dispatcher = self.dispatcher.clone();
    let mailbox = self.mailbox.clone();
    let schedule = move || {
      // The boolean is best-effort: a busy mailbox is fine, the next send
      // will retry. A failed submit is already logged by the shared wrapper.
      let _scheduled = dispatcher.register_for_execution(&mailbox, true, false);
    };
    Ok(SendOutcome::Schedule(Box::new(schedule)))
  }
}

#[allow(dead_code)]
fn _ensure_enqueue_outcome_used(_outcome: EnqueueOutcome) {}
