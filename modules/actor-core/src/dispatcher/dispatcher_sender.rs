#[cfg(test)]
mod tests;

use core::{pin::Pin, task::Context};

use cellactor_utils_core_rs::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use super::base::DispatcherGeneric;
use crate::{
  RuntimeToolbox,
  actor_prim::actor_ref::ActorRefSender,
  error::SendError,
  mailbox::{EnqueueOutcome, MailboxGeneric, MailboxOfferFutureGeneric, ScheduleHints},
  messaging::AnyMessageGeneric,
};

/// Sender that enqueues messages via actor handle.
pub struct DispatcherSenderGeneric<TB: RuntimeToolbox + 'static> {
  dispatcher: DispatcherGeneric<TB>,
  mailbox:    ArcShared<MailboxGeneric<TB>>,
}

/// Type alias for the default dispatcher sender.
pub type DispatcherSender = DispatcherSenderGeneric<NoStdToolbox>;

unsafe impl<TB: RuntimeToolbox + 'static> Send for DispatcherSenderGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for DispatcherSenderGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> DispatcherSenderGeneric<TB> {
  #[must_use]
  /// Creates a sender bound to the specified dispatcher.
  pub fn new(dispatcher: DispatcherGeneric<TB>) -> Self {
    let mailbox = dispatcher.mailbox();
    Self { dispatcher, mailbox }
  }

  fn poll_pending(&self, future: &mut MailboxOfferFutureGeneric<TB>) -> Result<(), SendError<TB>> {
    let waker = self.dispatcher.create_waker();
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
          block_hint();
        },
      }
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for DispatcherSenderGeneric<TB> {
  fn send(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    match self.mailbox.enqueue_user(message) {
      | Ok(EnqueueOutcome::Enqueued) => {
        self.dispatcher.register_for_execution(ScheduleHints {
          has_system_messages: false,
          has_user_messages:   true,
          backpressure_active: false,
        });
        Ok(())
      },
      | Ok(EnqueueOutcome::Pending(mut future)) => {
        self.dispatcher.register_for_execution(ScheduleHints {
          has_system_messages: false,
          has_user_messages:   true,
          backpressure_active: false,
        });
        self.poll_pending(&mut future)?;
        self.dispatcher.register_for_execution(ScheduleHints {
          has_system_messages: false,
          has_user_messages:   true,
          backpressure_active: false,
        });
        Ok(())
      },
      | Err(error) => Err(error),
    }
  }
}

pub(super) fn block_hint() {
  core::hint::spin_loop();
}
