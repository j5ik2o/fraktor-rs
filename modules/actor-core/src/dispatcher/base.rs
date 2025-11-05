use core::task::Waker;

use cellactor_utils_core_rs::sync::ArcShared;

use super::{
  dispatch_executor::DispatchExecutor, dispatch_shared::DispatchShared, dispatcher_core::DispatcherCore,
  dispatcher_state::DispatcherState, inline_executor::InlineExecutor, schedule_waker::ScheduleWaker,
};
use crate::{
  NoStdToolbox, RuntimeToolbox,
  error::SendError,
  mailbox::MailboxGeneric,
  messaging::{AnyMessageGeneric, SystemMessage, message_invoker::MessageInvoker},
};

/// Dispatcher that manages mailbox processing.
pub struct DispatcherGeneric<TB: RuntimeToolbox + 'static> {
  core: ArcShared<DispatcherCore<TB>>,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for DispatcherGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for DispatcherGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> DispatcherGeneric<TB> {
  /// Creates a new dispatcher from a mailbox and execution strategy.
  #[must_use]
  pub fn new(mailbox: ArcShared<MailboxGeneric<TB>>, executor: ArcShared<dyn DispatchExecutor<TB>>) -> Self {
    let throughput = mailbox.throughput_limit();
    let core = ArcShared::new(DispatcherCore::new(mailbox, executor, throughput));
    Self::from_core(core)
  }

  /// Creates a dispatcher using an inline execution strategy.
  #[must_use]
  pub fn with_inline_executor(mailbox: ArcShared<MailboxGeneric<TB>>) -> Self {
    Self::new(mailbox, ArcShared::new(InlineExecutor::<TB>::new()))
  }

  /// Registers an invoker.
  pub(crate) fn register_invoker(&self, invoker: ArcShared<dyn MessageInvoker<TB>>) {
    self.core.register_invoker(invoker);
  }

  /// Enqueues a user message.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is full or closed.
  #[allow(dead_code)]
  pub(crate) fn enqueue_user(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    DispatcherCore::enqueue_user(&self.core, message)
  }

  /// Enqueues a system message.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is full or closed.
  pub(crate) fn enqueue_system(&self, message: SystemMessage) -> Result<(), SendError<TB>> {
    DispatcherCore::enqueue_system(&self.core, message)
  }

  /// Requests execution from the scheduler.
  pub(crate) fn schedule(&self) {
    let should_run = {
      let core_ref = &*self.core;
      DispatcherState::compare_exchange(DispatcherState::Idle, DispatcherState::Running, core_ref.state()).is_ok()
    };

    if should_run {
      let executor = self.core.executor().clone();
      executor.execute(DispatchShared::new(self.core.clone()));
    }
  }

  /// Returns a reference to the mailbox.
  #[must_use]
  pub(crate) fn mailbox(&self) -> ArcShared<MailboxGeneric<TB>> {
    self.core.mailbox().clone()
  }

  /// Creates a waker for mailbox waiting.
  #[must_use]
  pub(crate) fn create_waker(&self) -> Waker {
    ScheduleWaker::<TB>::into_waker(self.core.clone())
  }

  pub(super) const fn from_core(core: ArcShared<DispatcherCore<TB>>) -> Self {
    Self { core }
  }

  /// Constructs an `ActorRefSender` implementation with a shared handle.
  #[must_use]
  #[allow(clippy::wrong_self_convention)]
  pub(crate) fn into_sender(&self) -> ArcShared<super::dispatcher_sender::DispatcherSender<TB>> {
    ArcShared::new(super::dispatcher_sender::DispatcherSender::new(self.clone()))
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for DispatcherGeneric<TB> {
  fn clone(&self) -> Self {
    Self { core: self.core.clone() }
  }
}

/// Type alias for `DispatcherGeneric` with the default `NoStdToolbox`.
pub type Dispatcher = DispatcherGeneric<NoStdToolbox>;
