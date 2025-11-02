use core::{pin::Pin, task::Context};

use cellactor_utils_core_rs::sync::ArcShared;

use super::dispatcher_struct::Dispatcher;
use crate::{
  actor_ref::ActorRefSender,
  any_message::AnyMessage,
  mailbox::{EnqueueOutcome, Mailbox},
  send_error::SendError,
};

/// Sender that enqueues messages via actor handle.
pub struct DispatcherSender {
  dispatcher: Dispatcher,
  mailbox:    ArcShared<Mailbox>,
}

impl DispatcherSender {
  #[must_use]
  /// Creates a sender bound to the specified dispatcher.
  pub fn new(dispatcher: Dispatcher) -> Self {
    let mailbox = dispatcher.mailbox();
    Self { dispatcher, mailbox }
  }

  fn poll_pending(&self, future: &mut crate::mailbox::MailboxOfferFuture) -> Result<(), SendError> {
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

impl ActorRefSender for DispatcherSender {
  fn send(&self, message: AnyMessage) -> Result<(), SendError> {
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

impl Dispatcher {
  /// Constructs an `ActorRefSender` implementation with a shared handle.
  #[must_use]
  pub fn into_sender(&self) -> ArcShared<DispatcherSender> {
    ArcShared::new(DispatcherSender::new(self.clone()))
  }
}
