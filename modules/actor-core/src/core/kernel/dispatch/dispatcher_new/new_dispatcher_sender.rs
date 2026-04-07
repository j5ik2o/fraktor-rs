//! `ActorRefSender` implementation backed by the new `MessageDispatcherShared`.
//!
//! `NewDispatcherSender` is constructed in `ActorCell::create` whenever the
//! actor system has a `dispatcher_new` configurator registered for the
//! resolved dispatcher id. The sender enqueues envelopes directly into the
//! receiver mailbox and asks the new dispatcher to schedule it.
//!
//! Backpressure: when the mailbox is full, `Mailbox::enqueue_envelope`
//! returns `EnqueueOutcome::Pending(future)`. The sender polls the
//! future to completion using a [`dispatcher_waker`](super::dispatcher_waker)
//! so that capacity-available signals trigger a mailbox re-schedule through
//! the new dispatcher tree.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_rs::core::sync::ArcShared;

use super::{dispatcher_waker::dispatcher_waker, message_dispatcher_shared::MessageDispatcherShared};
use crate::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  dispatch::mailbox::{EnqueueOutcome, Envelope, Mailbox, MailboxOfferFuture},
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

  /// Drives the [`MailboxOfferFuture`] to completion using a [`dispatcher_waker`].
  ///
  /// Each `Pending` poll first re-registers the mailbox for execution on the
  /// new dispatcher so the drain loop has a chance to free capacity. When
  /// the waker fires (or the queue succeeds) the future completes and we
  /// return to the send path.
  fn drive_offer_future(&self, mut future: MailboxOfferFuture) -> Result<(), SendError> {
    let waker = dispatcher_waker(self.dispatcher.clone(), self.mailbox.clone());
    let mut cx = Context::from_waker(&waker);
    loop {
      match Pin::new(&mut future).poll(&mut cx) {
        | Poll::Ready(Ok(())) => return Ok(()),
        | Poll::Ready(Err(error)) => return Err(error),
        | Poll::Pending => {
          // Nudge the dispatcher so the drain loop has a chance to free capacity.
          let _scheduled = self.dispatcher.register_for_execution(&self.mailbox, true, false);
        },
      }
    }
  }
}

impl ActorRefSender for NewDispatcherSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let envelope = Envelope::new(message);
    match self.mailbox.enqueue_envelope(envelope)? {
      | EnqueueOutcome::Enqueued => {},
      | EnqueueOutcome::Pending(future) => {
        self.drive_offer_future(future)?;
      },
    }
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
