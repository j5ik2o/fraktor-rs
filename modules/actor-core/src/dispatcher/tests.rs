use alloc::{vec, vec::Vec};
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use cellactor_utils_core_rs::{
  runtime_toolbox::NoStdToolbox,
  sync::{ArcShared, NoStdMutex, sync_mutex_like::SpinSyncMutex},
};

use crate::{
  dispatcher::{DispatchError, DispatchExecutor, DispatchSharedGeneric, DispatcherGeneric, TickExecutorGeneric},
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber},
  logging::LogLevel,
  mailbox::{MailboxGeneric, MailboxInstrumentation, MailboxPolicy, ScheduleHints},
  messaging::{AnyMessage, message_invoker::MessageInvoker},
  system::SystemState,
};

fn register_user_hint() -> ScheduleHints {
  ScheduleHints { has_system_messages: false, has_user_messages: true, backpressure_active: false }
}

fn system_instrumented_mailbox() -> (ArcShared<MailboxGeneric<NoStdToolbox>>, ArcShared<SystemState>) {
  let mailbox = ArcShared::new(MailboxGeneric::new(MailboxPolicy::unbounded(None)));
  let system = ArcShared::new(SystemState::new());
  let pid = system.allocate_pid();
  let instrumentation = MailboxInstrumentation::new(system.clone(), pid, None, None, None);
  mailbox.set_instrumentation(instrumentation);
  (mailbox, system)
}

#[test]
fn register_for_execution_schedules_once_until_idle() {
  let (mailbox, system) = system_instrumented_mailbox();
  let executor = ArcShared::new(RecordingExecutor::default());
  let dispatcher = dispatcher_with_executor(mailbox, executor.clone(), None, None);

  dispatcher.register_for_execution(register_user_hint());
  dispatcher.register_for_execution(register_user_hint());
  assert_eq!(executor.calls(), 1);

  executor.run_next();
  dispatcher.register_for_execution(register_user_hint());
  executor.run_next();
  assert_eq!(executor.calls(), 2);

  assert!(system.dead_letters().is_empty());
}

#[test]
fn rejected_execution_is_retried_and_logged_on_failure() {
  let (mailbox, system) = system_instrumented_mailbox();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl = ArcShared::new(EventRecorder::new(events.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&system.event_stream(), &subscriber);

  let executor = ArcShared::new(FlakyExecutor::new(vec![DispatchError::RejectedExecution; 3]));
  let dispatcher = dispatcher_with_executor(mailbox, executor.clone(), None, None);

  dispatcher.register_for_execution(register_user_hint());
  executor.assert_attempts(3);
  let logged = events
    .lock()
    .iter()
    .any(|event| matches!(event, crate::event_stream::EventStreamEvent::Log(log) if log.level() == LogLevel::Error));
  assert!(logged, "expected rejection log entry");
}

#[test]
fn dispatcher_respects_throughput_and_deadline_limits() {
  let mailbox = ArcShared::new(MailboxGeneric::new(
    MailboxPolicy::bounded(NonZeroUsize::new(2).unwrap(), crate::mailbox::MailboxOverflowStrategy::DropNewest, None)
      .with_throughput_limit(Some(NonZeroUsize::new(1).unwrap())),
  ));
  let system = ArcShared::new(SystemState::new());
  let pid = system.allocate_pid();
  mailbox.set_instrumentation(MailboxInstrumentation::new(system.clone(), pid, None, None, None));

  let executor = ArcShared::new(TickExecutorGeneric::new());
  let dispatcher = dispatcher_with_executor(mailbox.clone(), executor.clone(), Some(Duration::from_millis(1)), None);
  dispatcher.register_invoker(ArcShared::new(RecordingInvoker::default()));

  mailbox.enqueue_user(AnyMessage::new(1usize)).unwrap();
  mailbox.enqueue_user(AnyMessage::new(2usize)).unwrap();

  dispatcher.register_for_execution(register_user_hint());
  assert_eq!(executor.pending_tasks(), 1);
  executor.tick();
  assert!(executor.pending_tasks() <= 1);
}

fn dispatcher_with_executor(
  mailbox: ArcShared<MailboxGeneric<NoStdToolbox>>,
  executor: ArcShared<dyn DispatchExecutor<NoStdToolbox>>,
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
) -> DispatcherGeneric<NoStdToolbox> {
  DispatcherGeneric::with_executor(mailbox, executor, throughput_deadline, starvation_deadline)
}

struct RecordingExecutor {
  tasks: SpinSyncMutex<Vec<DispatchSharedGeneric<NoStdToolbox>>>,
  calls: AtomicUsize,
}

impl RecordingExecutor {
  fn new() -> Self {
    Self { tasks: SpinSyncMutex::new(Vec::new()), calls: AtomicUsize::new(0) }
  }

  fn run_next(&self) {
    if let Some(task) = self.tasks.lock().pop() {
      task.drive();
    }
  }

  fn calls(&self) -> usize {
    self.calls.load(Ordering::Acquire)
  }
}

impl Default for RecordingExecutor {
  fn default() -> Self {
    Self::new()
  }
}

impl DispatchExecutor<NoStdToolbox> for RecordingExecutor {
  fn execute(&self, dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
    self.calls.fetch_add(1, Ordering::Release);
    self.tasks.lock().push(dispatcher);
    Ok(())
  }
}

struct FlakyExecutor {
  failures: SpinSyncMutex<Vec<DispatchError>>,
  attempts: AtomicUsize,
}

impl FlakyExecutor {
  fn new(failures: Vec<DispatchError>) -> Self {
    Self { failures: SpinSyncMutex::new(failures), attempts: AtomicUsize::new(0) }
  }

  fn assert_attempts(&self, expected: usize) {
    assert_eq!(self.attempts.load(Ordering::Acquire), expected);
  }
}

impl DispatchExecutor<NoStdToolbox> for FlakyExecutor {
  fn execute(&self, _dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
    self.attempts.fetch_add(1, Ordering::Release);
    self.failures.lock().pop().map_or_else(|| Ok(()), Err)
  }
}

struct RecordingInvoker {
  messages: NoStdMutex<Vec<usize>>,
}

impl Default for RecordingInvoker {
  fn default() -> Self {
    Self { messages: NoStdMutex::new(Vec::new()) }
  }
}

impl MessageInvoker<NoStdToolbox> for RecordingInvoker {
  fn invoke_user_message(&self, message: AnyMessage) -> Result<(), crate::error::ActorError> {
    if let Some(value) = message.payload().downcast_ref::<usize>() {
      self.messages.lock().push(*value);
    }
    Ok(())
  }

  fn invoke_system_message(&self, _message: crate::messaging::SystemMessage) -> Result<(), crate::error::ActorError> {
    Ok(())
  }
}

struct EventRecorder {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl EventRecorder {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for EventRecorder {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}
