#[cfg(test)]
mod tests;

use core::{pin::Pin, task::Context};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use super::base::DispatcherGeneric;
use crate::core::{
  actor_prim::actor_ref::ActorRefSender,
  dispatcher::ScheduleAdapter,
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

  fn poll_pending(
    &self,
    adapter: &ArcShared<dyn ScheduleAdapter<TB>>,
    future: &mut MailboxOfferFutureGeneric<TB>,
  ) -> Result<(), SendError<TB>> {
    let waker = adapter.create_waker(self.dispatcher.clone());
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
          adapter.on_pending();
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
        let adapter = self.dispatcher.schedule_adapter();
        adapter.on_pending();
        self.dispatcher.register_for_execution(ScheduleHints {
          has_system_messages: false,
          has_user_messages:   true,
          backpressure_active: false,
        });
        self.poll_pending(&adapter, &mut future)?;
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
