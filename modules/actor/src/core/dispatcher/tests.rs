extern crate std;

use alloc::{vec, vec::Vec};
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};
use std::thread;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use super::schedule_waker::ScheduleWaker;
use crate::core::{
  actor_prim::actor_ref::ActorRefSender,
  dispatcher::{
    DispatchError, DispatchExecutor, DispatchSharedGeneric, DispatcherGeneric, ScheduleAdapter, TickExecutorGeneric,
  },
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber},
  logging::LogLevel,
  mailbox::{
    EnqueueOutcome, MailboxGeneric, MailboxInstrumentation, MailboxOverflowStrategy, MailboxPolicy, ScheduleHints,
  },
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

fn bounded_mailbox(capacity: usize) -> (ArcShared<MailboxGeneric<NoStdToolbox>>, ArcShared<SystemState>) {
  let policy =
    MailboxPolicy::bounded(NonZeroUsize::new(capacity).expect("capacity"), MailboxOverflowStrategy::Block, None);
  let mailbox = ArcShared::new(MailboxGeneric::new(policy));
  let system = ArcShared::new(SystemState::new());
  let pid = system.allocate_pid();
  let instrumentation = MailboxInstrumentation::new(system.clone(), pid, Some(capacity), None, None);
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
  let logged = events.lock().iter().any(
    |event| matches!(event, crate::core::event_stream::EventStreamEvent::Log(log) if log.level() == LogLevel::Error),
  );
  assert!(logged, "expected rejection log entry");
}

#[test]
fn dispatcher_respects_throughput_and_deadline_limits() {
  let mailbox = ArcShared::new(MailboxGeneric::new(
    MailboxPolicy::bounded(
      NonZeroUsize::new(2).unwrap(),
      crate::core::mailbox::MailboxOverflowStrategy::DropNewest,
      None,
    )
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

#[test]
fn schedule_adapter_receives_pending_signal() {
  let (mailbox, _system) = bounded_mailbox(1);
  let executor = ArcShared::new(TickExecutorGeneric::new());
  let adapter = ArcShared::new(CountingScheduleAdapter::default());
  let dispatcher = dispatcher_with_executor_and_adapter(mailbox.clone(), executor.clone(), None, None, adapter.clone());
  dispatcher.register_invoker(ArcShared::new(RecordingInvoker::default()));

  mailbox.enqueue_user(AnyMessage::new(1usize)).expect("first message");
  let sender = dispatcher.into_sender();

  let handle = thread::spawn(move || {
    sender.send(AnyMessage::new(2usize)).expect("second message");
  });

  thread::sleep(Duration::from_millis(1));
  dispatcher.register_for_execution(register_user_hint());
  executor.tick();
  handle.join().expect("join");

  assert!(adapter.pending_calls() > 0);
}

#[test]
fn schedule_adapter_notified_on_rejection() {
  let (mailbox, system) = system_instrumented_mailbox();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl = ArcShared::new(EventRecorder::new(events.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&system.event_stream(), &subscriber);

  let executor = ArcShared::new(FlakyExecutor::new(vec![DispatchError::RejectedExecution; 3]));
  let adapter = ArcShared::new(CountingScheduleAdapter::default());
  let dispatcher = dispatcher_with_executor_and_adapter(mailbox, executor.clone(), None, None, adapter.clone());

  dispatcher.register_for_execution(register_user_hint());
  executor.assert_attempts(3);
  assert!(adapter.rejected_calls() >= 1);

  let logged = events.lock().iter().any(
    |event| matches!(event, crate::core::event_stream::EventStreamEvent::Log(log) if log.level() == LogLevel::Error),
  );
  assert!(logged, "expected rejection log entry");
}

#[test]
fn dispatcher_dump_event_published() {
  let (mailbox, system) = system_instrumented_mailbox();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl = ArcShared::new(EventRecorder::new(events.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&system.event_stream(), &subscriber);

  let executor = ArcShared::new(RecordingExecutor::default());
  let adapter = ArcShared::new(CountingScheduleAdapter::default());
  let dispatcher = dispatcher_with_executor_and_adapter(mailbox, executor, None, None, adapter);

  dispatcher.publish_dump_metrics();

  assert!(events.lock().iter().any(|event| matches!(event, EventStreamEvent::DispatcherDump(_))));
}

#[test]
fn telemetry_captures_mailbox_pressure_and_dispatcher_dump() {
  let (mailbox, system) = bounded_mailbox(2);
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl = ArcShared::new(EventRecorder::new(events.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&system.event_stream(), &subscriber);

  let executor = ArcShared::new(RecordingExecutor::default());
  let adapter = ArcShared::new(CountingScheduleAdapter::default());
  let dispatcher = dispatcher_with_executor_and_adapter(mailbox.clone(), executor, None, None, adapter);
  dispatcher.register_invoker(ArcShared::new(RecordingInvoker::default()));

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(1usize)), Ok(EnqueueOutcome::Enqueued)));
  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(2usize)), Ok(EnqueueOutcome::Enqueued)));
  dispatcher.publish_dump_metrics();

  let guard = events.lock();
  assert!(guard.iter().any(|event| matches!(event, EventStreamEvent::MailboxPressure(_))));
  assert!(guard.iter().any(|event| matches!(event, EventStreamEvent::DispatcherDump(_))));
}

fn dispatcher_with_executor(
  mailbox: ArcShared<MailboxGeneric<NoStdToolbox>>,
  executor: ArcShared<dyn DispatchExecutor<NoStdToolbox>>,
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
) -> DispatcherGeneric<NoStdToolbox> {
  let adapter = ArcShared::new(crate::core::dispatcher::InlineScheduleAdapter::new());
  dispatcher_with_executor_and_adapter(mailbox, executor, throughput_deadline, starvation_deadline, adapter)
}

fn dispatcher_with_executor_and_adapter(
  mailbox: ArcShared<MailboxGeneric<NoStdToolbox>>,
  executor: ArcShared<dyn DispatchExecutor<NoStdToolbox>>,
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
  adapter: ArcShared<dyn ScheduleAdapter<NoStdToolbox>>,
) -> DispatcherGeneric<NoStdToolbox> {
  DispatcherGeneric::with_adapter(mailbox, executor, adapter, throughput_deadline, starvation_deadline)
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
  fn invoke_user_message(&self, message: AnyMessage) -> Result<(), crate::core::error::ActorError> {
    if let Some(value) = message.payload().downcast_ref::<usize>() {
      self.messages.lock().push(*value);
    }
    Ok(())
  }

  fn invoke_system_message(
    &self,
    _message: crate::core::messaging::SystemMessage,
  ) -> Result<(), crate::core::error::ActorError> {
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

struct CountingScheduleAdapter {
  pending:  ArcShared<NoStdMutex<usize>>,
  rejected: ArcShared<NoStdMutex<usize>>,
}

impl CountingScheduleAdapter {
  fn new() -> Self {
    Self { pending: ArcShared::new(NoStdMutex::new(0)), rejected: ArcShared::new(NoStdMutex::new(0)) }
  }

  fn pending_calls(&self) -> usize {
    *self.pending.lock()
  }

  fn rejected_calls(&self) -> usize {
    *self.rejected.lock()
  }
}

impl Default for CountingScheduleAdapter {
  fn default() -> Self {
    Self::new()
  }
}

impl ScheduleAdapter<NoStdToolbox> for CountingScheduleAdapter {
  fn create_waker(&self, dispatcher: DispatcherGeneric<NoStdToolbox>) -> core::task::Waker {
    ScheduleWaker::<NoStdToolbox>::into_waker(dispatcher)
  }

  fn on_pending(&self) {
    *self.pending.lock() += 1;
  }

  fn notify_rejected(&self, _attempts: usize) {
    *self.rejected.lock() += 1;
  }
}
