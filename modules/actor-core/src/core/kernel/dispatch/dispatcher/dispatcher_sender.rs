//! `ActorRefSender` implementation backed by `MessageDispatcherShared`.
//!
//! `DispatcherSender` is constructed in `ActorCell::create` whenever the
//! actor system has a dispatcher configurator registered for the resolved
//! dispatcher id. It routes every `ActorRef::tell` through
//! `MessageDispatcherShared::dispatch` so the dispatcher's own `dispatch`
//! hook (default: enqueue into the receiver mailbox;
//! `BalancingDispatcher`: enqueue into the shared team queue) decides where
//! the envelope lands. Bypassing the trait and enqueuing directly on
//! `receiver.mailbox` would break `BalancingDispatcher` load balancing.
//!
//! The sender holds only the receiver mailbox. The owning [`ActorCell`] is
//! resolved via `Mailbox::actor()` on each `send`, which avoids an
//! `ActorCell -> sender -> ActorCell` ownership cycle.

#[cfg(test)]
mod tests;

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
    // reference. We must route through the dispatcher's own `dispatch` hook
    // (rather than enqueuing directly on the mailbox) so `BalancingDispatcher`
    // can intercept the envelope and route it into its shared team queue.
    // `ActorCell::create` installs the weak handle on the mailbox before the
    // cell becomes observable externally, so this upgrade only fails after
    // the cell has been dropped, at which point reporting `closed` is the
    // correct answer.
    let Some(cell) = self.mailbox.actor().and_then(|weak| weak.upgrade()) else {
      return Err(SendError::closed(envelope.into_payload()));
    };
    self.dispatcher.dispatch(&cell, envelope)?;
    Ok(SendOutcome::Delivered)
  }
}
