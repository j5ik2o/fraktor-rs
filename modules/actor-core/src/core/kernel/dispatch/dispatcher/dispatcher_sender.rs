#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::{pin::Pin, task::Context};

use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess};

use super::dispatcher_shared::DispatcherShared;
use crate::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  dispatch::{
    dispatcher::schedule_adapter_shared::ScheduleAdapterShared,
    mailbox::{EnqueueOutcome, Mailbox, MailboxOfferFuture, ScheduleHints},
  },
};

/// Sender that enqueues messages via actor handle.
pub struct DispatcherSender {
  dispatcher: DispatcherShared,
  mailbox:    ArcShared<Mailbox>,
}

unsafe impl Send for DispatcherSender {}
unsafe impl Sync for DispatcherSender {}

impl DispatcherSender {
  #[must_use]
  /// Creates a sender bound to the specified dispatcher.
  pub fn new(dispatcher: DispatcherShared) -> Self {
    let mailbox = dispatcher.mailbox();
    Self { dispatcher, mailbox }
  }

  fn poll_pending(&self, adapter: &ScheduleAdapterShared, future: &mut MailboxOfferFuture) -> Result<(), SendError> {
    let waker = adapter.with_write(|a| a.create_waker(self.dispatcher.clone()));
    let mut cx = Context::from_waker(&waker);

    loop {
      match Pin::new(&mut *future).poll(&mut cx) {
        | core::task::Poll::Ready(Ok(_)) => return Ok(()),
        | core::task::Poll::Ready(Err(error)) => return Err(error),
        | core::task::Poll::Pending => {
          self.dispatcher.register_for_execution(ScheduleHints {
            has_system_messages: false,
            has_user_messages:   true,
            backpressure_active: false,
          });
          adapter.with_write(|a| a.on_pending());
        },
      }
    }
  }

  fn schedule_user_execution(&self) -> SendOutcome {
    let dispatcher = self.dispatcher.clone();
    let schedule = move || {
      dispatcher.register_for_execution(ScheduleHints {
        has_system_messages: false,
        has_user_messages:   true,
        backpressure_active: false,
      });
    };
    SendOutcome::Schedule(Box::new(schedule))
  }
}

impl ActorRefSender for DispatcherSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    match self.mailbox.enqueue_user(message) {
      | Ok(EnqueueOutcome::Enqueued) => Ok(self.schedule_user_execution()),
      | Ok(EnqueueOutcome::Pending(mut future)) => {
        let adapter = self.dispatcher.schedule_adapter();
        adapter.with_write(|a| a.on_pending());
        self.poll_pending(&adapter, &mut future)?;
        Ok(self.schedule_user_execution())
      },
      | Err(error) => Err(error),
    }
  }
}
