#[cfg(test)]
mod tests;

use core::{pin::Pin, task::Context};

use cellactor_utils_core_rs::sync::ArcShared;

use super::base::Dispatcher;
use crate::{
  RuntimeToolbox,
  actor_prim::actor_ref::ActorRefSender,
  error::SendError,
  mailbox::{EnqueueOutcome, Mailbox, MailboxOfferFuture},
  messaging::AnyMessage,
};

/// Sender that enqueues messages via actor handle.
pub struct DispatcherSender<TB: RuntimeToolbox + 'static> {
  dispatcher: Dispatcher<TB>,
  mailbox:    ArcShared<Mailbox<TB>>,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for DispatcherSender<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for DispatcherSender<TB> {}

impl<TB: RuntimeToolbox + 'static> DispatcherSender<TB> {
  #[must_use]
  /// Creates a sender bound to the specified dispatcher.
  pub fn new(dispatcher: Dispatcher<TB>) -> Self {
    let mailbox = dispatcher.mailbox();
    Self { dispatcher, mailbox }
  }

  fn poll_pending(&self, future: &mut MailboxOfferFuture<TB>) -> Result<(), SendError<TB>> {
    let waker = self.dispatcher.create_waker();
    let mut cx = Context::from_waker(&waker);

    loop {
      match Pin::new(&mut *future).poll(&mut cx) {
        | core::task::Poll::Ready(Ok(_)) => return Ok(()),
        | core::task::Poll::Ready(Err(error)) => return Err(error),
        | core::task::Poll::Pending => {
          self.dispatcher.schedule();
          block_hint();
        },
      }
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for DispatcherSender<TB> {
  fn send(&self, message: AnyMessage<TB>) -> Result<(), SendError<TB>> {
    match self.mailbox.enqueue_user(message) {
      | Ok(EnqueueOutcome::Enqueued) => {
        self.dispatcher.schedule();
        Ok(())
      },
      | Ok(EnqueueOutcome::Pending(mut future)) => {
        self.dispatcher.schedule();
        self.poll_pending(&mut future)?;
        self.dispatcher.schedule();
        Ok(())
      },
      | Err(error) => Err(error),
    }
  }
}

pub(super) fn block_hint() {
  core::hint::spin_loop();
}
