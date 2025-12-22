#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::{pin::Pin, task::Context};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::{ArcShared, SharedAccess},
};

use super::base::DispatcherSharedGeneric;
use crate::core::{
  actor_prim::actor_ref::{ActorRefSender, SendOutcome},
  dispatcher::schedule_adapter_shared::ScheduleAdapterSharedGeneric,
  error::SendError,
  mailbox::{EnqueueOutcome, MailboxGeneric, MailboxOfferFutureGeneric, ScheduleHints},
  messaging::AnyMessageGeneric,
};

/// Sender that enqueues messages via actor handle.
pub struct DispatcherSenderGeneric<TB: RuntimeToolbox + 'static> {
  dispatcher: DispatcherSharedGeneric<TB>,
  mailbox:    ArcShared<MailboxGeneric<TB>>,
}

/// Type alias for the default dispatcher sender.
pub type DispatcherSender = DispatcherSenderGeneric<NoStdToolbox>;

unsafe impl<TB: RuntimeToolbox + 'static> Send for DispatcherSenderGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for DispatcherSenderGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> DispatcherSenderGeneric<TB> {
  #[must_use]
  /// Creates a sender bound to the specified dispatcher.
  pub fn new(dispatcher: DispatcherSharedGeneric<TB>) -> Self {
    let mailbox = dispatcher.mailbox();
    Self { dispatcher, mailbox }
  }

  fn poll_pending(
    &self,
    adapter: &ScheduleAdapterSharedGeneric<TB>,
    future: &mut MailboxOfferFutureGeneric<TB>,
  ) -> Result<(), SendError<TB>> {
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
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for DispatcherSenderGeneric<TB> {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    match self.mailbox.enqueue_user(message) {
      | Ok(EnqueueOutcome::Enqueued) => {
        if self.mailbox.is_running() {
          self.dispatcher.register_for_execution(ScheduleHints {
            has_system_messages: false,
            has_user_messages:   true,
            backpressure_active: false,
          });
          return Ok(SendOutcome::Delivered);
        }

        let dispatcher = self.dispatcher.clone();
        let schedule = move || {
          dispatcher.register_for_execution(ScheduleHints {
            has_system_messages: false,
            has_user_messages:   true,
            backpressure_active: false,
          });
        };
        Ok(SendOutcome::Schedule(Box::new(schedule)))
      },
      | Ok(EnqueueOutcome::Pending(mut future)) => {
        let adapter = self.dispatcher.schedule_adapter();
        adapter.with_write(|a| a.on_pending());
        if self.mailbox.is_running() {
          self.poll_pending(&adapter, &mut future)?;
          return Ok(SendOutcome::Delivered);
        }

        self.poll_pending(&adapter, &mut future)?;
        let dispatcher = self.dispatcher.clone();
        let schedule = move || {
          dispatcher.register_for_execution(ScheduleHints {
            has_system_messages: false,
            has_user_messages:   true,
            backpressure_active: false,
          });
        };
        Ok(SendOutcome::Schedule(Box::new(schedule)))
      },
      | Err(error) => Err(error),
    }
  }
}
