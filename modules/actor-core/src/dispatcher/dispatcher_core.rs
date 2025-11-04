#[cfg(test)]
mod tests;

use core::{
  num::NonZeroUsize,
  pin::Pin,
  task::{Context, Poll},
};

use cellactor_utils_core_rs::sync::{ArcShared, SyncMutexFamily, sync_mutex_like::SyncMutexLike};
use portable_atomic::AtomicU8;

use super::{
  dispatch_executor::DispatchExecutor, dispatcher_sender::block_hint, dispatcher_state::DispatcherState,
  schedule_waker::ScheduleWaker,
};
use crate::{
  RuntimeToolbox, ToolboxMutex,
  error::{ActorError, SendError},
  mailbox::{EnqueueOutcome, Mailbox, MailboxMessage, MailboxOfferFuture},
  messaging::{AnyMessage, SystemMessage, message_invoker::MessageInvoker},
};

const DEFAULT_THROUGHPUT: usize = 300;

/// Entity that drains the mailbox and invokes messages.
pub(super) struct DispatcherCore<TB: RuntimeToolbox + 'static> {
  mailbox:          ArcShared<Mailbox<TB>>,
  executor:         ArcShared<dyn DispatchExecutor<TB>>,
  invoker:          ToolboxMutex<Option<ArcShared<dyn MessageInvoker<TB>>>, TB>,
  state:            AtomicU8,
  throughput_limit: Option<NonZeroUsize>,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for DispatcherCore<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for DispatcherCore<TB> {}

impl<TB: RuntimeToolbox + 'static> DispatcherCore<TB> {
  pub(super) fn new(
    mailbox: ArcShared<Mailbox<TB>>,
    executor: ArcShared<dyn DispatchExecutor<TB>>,
    throughput_limit: Option<NonZeroUsize>,
  ) -> Self {
    Self {
      mailbox,
      executor,
      invoker: <TB::MutexFamily as SyncMutexFamily>::create(None),
      state: AtomicU8::new(DispatcherState::Idle.as_u8()),
      throughput_limit,
    }
  }

  pub(super) const fn mailbox(&self) -> &ArcShared<Mailbox<TB>> {
    &self.mailbox
  }

  pub(super) fn register_invoker(&self, invoker: ArcShared<dyn MessageInvoker<TB>>) {
    *self.invoker.lock() = Some(invoker);
  }

  pub(super) fn executor(&self) -> &ArcShared<dyn DispatchExecutor<TB>> {
    &self.executor
  }

  pub(super) const fn state(&self) -> &AtomicU8 {
    &self.state
  }

  pub(super) fn drive(self_arc: &ArcShared<Self>) {
    loop {
      {
        let this = self_arc;
        this.process_batch();
      }

      let should_continue = {
        let this = self_arc;
        DispatcherState::Idle.store(&this.state);
        this.has_pending_work()
          && DispatcherState::compare_exchange(DispatcherState::Idle, DispatcherState::Running, &this.state).is_ok()
      };

      if !should_continue {
        break;
      }
    }
  }

  fn process_batch(&self) {
    let limit = self.throughput_limit.map(NonZeroUsize::get).unwrap_or(DEFAULT_THROUGHPUT);
    let mut processed = 0_usize;

    while processed < limit {
      match self.mailbox.dequeue() {
        | Some(MailboxMessage::System(msg)) => {
          self.handle_system_message(msg);
          processed += 1;
        },
        | Some(MailboxMessage::User(msg)) => {
          self.handle_user_message(msg);
          processed += 1;
        },
        | None => break,
      }
    }
  }

  fn handle_system_message(&self, message: SystemMessage) {
    match message {
      | SystemMessage::Suspend => self.mailbox.suspend(),
      | SystemMessage::Resume => self.mailbox.resume(),
      | other => {
        let _ = self.invoke_system_message(other);
      },
    }
  }

  fn handle_user_message(&self, message: AnyMessage<TB>) {
    let _ = self.invoke_user_message(message);
  }

  fn invoke_user_message(&self, message: AnyMessage<TB>) -> Result<(), ActorError> {
    if let Some(invoker) = self.invoker.lock().as_ref() {
      return invoker.invoke_user_message(message);
    }
    Ok(())
  }

  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError> {
    if let Some(invoker) = self.invoker.lock().as_ref() {
      return invoker.invoke_system_message(message);
    }
    Ok(())
  }

  pub(super) fn enqueue_user(self_arc: &ArcShared<Self>, message: AnyMessage<TB>) -> Result<(), SendError<TB>> {
    match self_arc.mailbox.enqueue_user(message) {
      | Ok(EnqueueOutcome::Enqueued) => {
        super::base::Dispatcher::from_core(self_arc.clone()).schedule();
        Ok(())
      },
      | Ok(EnqueueOutcome::Pending(mut future)) => {
        Self::drain_offer_future(self_arc, &mut future)?;
        super::base::Dispatcher::from_core(self_arc.clone()).schedule();
        Ok(())
      },
      | Err(error) => Err(error),
    }
  }

  pub(super) fn enqueue_system(self_arc: &ArcShared<Self>, message: SystemMessage) -> Result<(), SendError<TB>> {
    self_arc.mailbox.enqueue_system(message)?;
    super::base::Dispatcher::from_core(self_arc.clone()).schedule();
    Ok(())
  }

  fn drain_offer_future(self_arc: &ArcShared<Self>, future: &mut MailboxOfferFuture<TB>) -> Result<(), SendError<TB>> {
    let waker = ScheduleWaker::<TB>::into_waker(self_arc.clone());
    let mut cx = Context::from_waker(&waker);

    loop {
      match Pin::new(&mut *future).poll(&mut cx) {
        | Poll::Ready(Ok(_)) => return Ok(()),
        | Poll::Ready(Err(error)) => return Err(error),
        | Poll::Pending => {
          super::base::Dispatcher::from_core(self_arc.clone()).schedule();
          block_hint();
        },
      }
    }
  }

  fn has_pending_work(&self) -> bool {
    self.mailbox.system_len() > 0 || (!self.mailbox.is_suspended() && self.mailbox.user_len() > 0)
  }
}
