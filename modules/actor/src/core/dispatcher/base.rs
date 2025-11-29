use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use super::{
  DispatcherSenderGeneric,
  dispatch_error::DispatchError,
  dispatch_executor::DispatchExecutor,
  dispatch_executor_runner::DispatchExecutorRunner,
  dispatch_shared::DispatchSharedGeneric,
  dispatcher_core::{DispatcherCore, MAX_EXECUTOR_RETRIES},
  dispatcher_state::DispatcherState,
  inline_executor::InlineExecutorGeneric,
  inline_schedule_adapter::InlineScheduleAdapter,
  schedule_adapter::ScheduleAdapter,
};
use crate::core::{
  error::SendError,
  mailbox::{MailboxGeneric, MailboxPressureEvent, ScheduleHints},
  messaging::{AnyMessageGeneric, SystemMessage, message_invoker::MessageInvokerShared},
};

/// Dispatcher that manages mailbox processing.
pub struct DispatcherGeneric<TB: RuntimeToolbox + 'static> {
  core: ArcShared<DispatcherCore<TB>>,
}

/// Type alias for `DispatcherGeneric` with the default `NoStdToolbox`.
pub type Dispatcher = DispatcherGeneric<NoStdToolbox>;

unsafe impl<TB: RuntimeToolbox + 'static> Send for DispatcherGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for DispatcherGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> DispatcherGeneric<TB> {
  /// Creates a new dispatcher from a mailbox and execution strategy.
  #[must_use]
  pub fn new(mailbox: ArcShared<MailboxGeneric<TB>>, executor: ArcShared<DispatchExecutorRunner<TB>>) -> Self {
    Self::with_executor(mailbox, executor, None, None)
  }

  /// Creates a dispatcher with explicit runtime limits.
  #[must_use]
  pub fn with_executor(
    mailbox: ArcShared<MailboxGeneric<TB>>,
    executor: ArcShared<DispatchExecutorRunner<TB>>,
    throughput_deadline: Option<Duration>,
    starvation_deadline: Option<Duration>,
  ) -> Self {
    let adapter = ArcShared::new(InlineScheduleAdapter::new());
    Self::with_adapter(mailbox, executor, adapter, throughput_deadline, starvation_deadline)
  }

  /// Creates a dispatcher with a custom schedule adapter.
  #[must_use]
  pub fn with_adapter(
    mailbox: ArcShared<MailboxGeneric<TB>>,
    executor: ArcShared<DispatchExecutorRunner<TB>>,
    schedule_adapter: ArcShared<dyn ScheduleAdapter<TB>>,
    throughput_deadline: Option<Duration>,
    starvation_deadline: Option<Duration>,
  ) -> Self {
    let throughput = mailbox.throughput_limit();
    let core = ArcShared::new(DispatcherCore::new(
      mailbox,
      executor,
      schedule_adapter,
      throughput,
      throughput_deadline,
      starvation_deadline,
    ));
    Self::from_core(core)
  }

  /// Creates a dispatcher using an inline execution strategy.
  #[must_use]
  pub fn with_inline_executor(mailbox: ArcShared<MailboxGeneric<TB>>) -> Self {
    let executor: Box<dyn DispatchExecutor<TB>> = Box::new(InlineExecutorGeneric::<TB>::new());
    let runner = ArcShared::new(DispatchExecutorRunner::new(executor));
    Self::new(mailbox, runner)
  }

  /// Registers an invoker.
  pub(crate) fn register_invoker(&self, invoker: MessageInvokerShared<TB>) {
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
  pub(crate) fn spawn_execution(&self) {
    let should_run = {
      let core_ref = &*self.core;
      DispatcherState::compare_exchange(DispatcherState::Idle, DispatcherState::Running, core_ref.state()).is_ok()
    };

    if should_run {
      self.try_execute(0);
    }
  }

  fn try_execute(&self, attempt: usize) {
    let executor = self.core.executor().clone();
    let task = DispatchSharedGeneric::new(self.core.clone());
    match executor.submit(task) {
      | Ok(()) => {},
      | Err(DispatchError::RejectedExecution) if attempt < MAX_EXECUTOR_RETRIES => {
        self.try_execute(attempt + 1);
      },
      | Err(error) => {
        self.core.handle_executor_failure(attempt + 1, error);
      },
    }
  }

  /// Requests scheduling with the provided hints.
  pub fn register_for_execution(&self, hints: ScheduleHints) {
    DispatcherCore::request_execution(&self.core, hints);
  }

  /// Returns a reference to the mailbox.
  #[must_use]
  pub(crate) fn mailbox(&self) -> ArcShared<MailboxGeneric<TB>> {
    self.core.mailbox().clone()
  }

  /// Notifies the dispatcher about a mailbox pressure signal.
  pub(crate) fn notify_backpressure(&self, event: &MailboxPressureEvent) {
    DispatcherCore::handle_backpressure(&self.core, event);
  }

  pub(crate) const fn from_core(core: ArcShared<DispatcherCore<TB>>) -> Self {
    Self { core }
  }

  /// Constructs an `ActorRefSender` implementation with a shared handle.
  #[must_use]
  #[allow(clippy::wrong_self_convention)]
  pub(crate) fn into_sender(&self) -> ArcShared<DispatcherSenderGeneric<TB>> {
    ArcShared::new(DispatcherSenderGeneric::new(self.clone()))
  }

  pub(crate) fn schedule_adapter(&self) -> ArcShared<dyn ScheduleAdapter<TB>> {
    self.core.schedule_adapter()
  }

  /// Publishes dispatcher diagnostics to the event stream, when instrumentation is available.
  pub fn publish_dump_metrics(&self) {
    DispatcherCore::publish_dump(&self.core);
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for DispatcherGeneric<TB> {
  fn clone(&self) -> Self {
    Self { core: self.core.clone() }
  }
}
