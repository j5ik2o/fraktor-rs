use core::task::Waker;

use cellactor_utils_core_rs::sync::ArcShared;

use super::{
  dispatch_executor::DispatchExecutor, dispatch_handle::DispatchHandle, dispatcher_core::DispatcherCore,
  dispatcher_state::DispatcherState, inline_executor::InlineExecutor, schedule_waker::ScheduleWaker,
};
use crate::{any_message::AnyMessage, mailbox::Mailbox, send_error::SendError, system_message::SystemMessage};

/// Dispatcher that manages mailbox processing.
pub struct Dispatcher {
  core: ArcShared<DispatcherCore>,
}

impl Dispatcher {
  /// Creates a new dispatcher from a mailbox and execution strategy.
  #[must_use]
  pub fn new(mailbox: ArcShared<Mailbox>, executor: ArcShared<dyn DispatchExecutor>) -> Self {
    let throughput = mailbox.throughput_limit();
    let core = ArcShared::new(DispatcherCore::new(mailbox, executor, throughput));
    Self::from_core(core)
  }

  /// Creates a dispatcher using an inline execution strategy.
  #[must_use]
  pub fn with_inline_executor(mailbox: ArcShared<Mailbox>) -> Self {
    Self::new(mailbox, ArcShared::new(InlineExecutor::new()))
  }

  /// Registers an invoker.
  pub fn register_invoker(&self, invoker: ArcShared<dyn crate::message_invoker::MessageInvoker>) {
    self.core.register_invoker(invoker);
  }

  /// Enqueues a user message.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is closed or full.
  pub fn enqueue_user(&self, message: AnyMessage) -> Result<(), SendError> {
    DispatcherCore::enqueue_user(&self.core, message)
  }

  /// Enqueues a system message.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is closed.
  pub fn enqueue_system(&self, message: SystemMessage) -> Result<(), SendError> {
    DispatcherCore::enqueue_system(&self.core, message)
  }

  /// Requests execution from the scheduler.
  pub fn schedule(&self) {
    let should_run = {
      let core_ref = &*self.core;
      DispatcherState::compare_exchange(DispatcherState::Idle, DispatcherState::Running, core_ref.state()).is_ok()
    };

    if should_run {
      let executor = self.core.executor().clone();
      executor.execute(DispatchHandle::new(self.core.clone()));
    }
  }

  /// Returns a reference to the mailbox.
  #[must_use]
  pub fn mailbox(&self) -> ArcShared<Mailbox> {
    self.core.mailbox().clone()
  }

  /// Creates a waker for mailbox waiting.
  #[must_use]
  pub fn create_waker(&self) -> Waker {
    ScheduleWaker::into_waker(self.core.clone())
  }

  pub(super) const fn from_core(core: ArcShared<DispatcherCore>) -> Self {
    Self { core }
  }
}

impl Clone for Dispatcher {
  fn clone(&self) -> Self {
    Self { core: self.core.clone() }
  }
}
