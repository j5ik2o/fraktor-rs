extern crate std;

use alloc::{boxed::Box, vec, vec::Vec};
use core::{
  any::Any,
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};
use std::thread;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SpinSyncMutex},
};

use super::schedule_waker::ScheduleWaker;
use crate::core::{
  dispatch::{
    dispatcher::{
      DispatchError, DispatchExecutor, DispatchExecutorRunner, DispatchSharedGeneric, DispatcherSharedGeneric,
      ScheduleAdapter, ScheduleAdapterSharedGeneric, TickExecutorGeneric,
    },
    mailbox::{
      EnqueueOutcome, MailboxGeneric, MailboxInstrumentation, MailboxOverflowStrategy, MailboxPolicy, ScheduleHints,
    },
  },
  event_stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  logging::LogLevel,
  messaging::{
    AnyMessage,
    message_invoker::{MessageInvoker, MessageInvokerShared},
  },
  system::{ActorSystem, SystemStateShared},
};

fn register_user_hint() -> ScheduleHints {
  ScheduleHints { has_system_messages: false, has_user_messages: true, backpressure_active: false }
}

fn system_instrumented_mailbox() -> (ArcShared<MailboxGeneric<NoStdToolbox>>, SystemStateShared) {
  let mailbox = ArcShared::new(MailboxGeneric::new(MailboxPolicy::unbounded(None)));
  let system = ActorSystem::new_empty().state();
  let pid = system.allocate_pid();
  let instrumentation = MailboxInstrumentation::new(system.clone(), pid, None, None, None);
  mailbox.set_instrumentation(instrumentation);
  (mailbox, system)
}

fn bounded_mailbox(capacity: usize) -> (ArcShared<MailboxGeneric<NoStdToolbox>>, SystemStateShared) {
  let policy =
    MailboxPolicy::bounded(NonZeroUsize::new(capacity).expect("capacity"), MailboxOverflowStrategy::Block, None);
  let mailbox = ArcShared::new(MailboxGeneric::new(policy));
  let system = ActorSystem::new_empty().state();
  let pid = system.allocate_pid();
  let instrumentation = MailboxInstrumentation::new(system.clone(), pid, Some(capacity), None, None);
  mailbox.set_instrumentation(instrumentation);
  (mailbox, system)
}

#[test]
fn register_for_execution_schedules_once_until_idle() {
  let (mailbox, system) = system_instrumented_mailbox();
  let (recording, runner) = recording_executor_with_runner();
  let dispatcher = dispatcher_with_executor(mailbox, runner, None, None);

  dispatcher.register_for_execution(register_user_hint());
  dispatcher.register_for_execution(register_user_hint());
  assert_eq!(recording.calls(), 1);

  recording.run_next();
  dispatcher.register_for_execution(register_user_hint());
  recording.run_next();
  assert_eq!(recording.calls(), 2);

  assert!(system.dead_letters().is_empty());
}

#[test]
fn rejected_execution_is_retried_and_logged_on_failure() {
  let (mailbox, system) = system_instrumented_mailbox();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(EventRecorder::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);

  let (flaky, runner) = flaky_executor_with_runner(vec![DispatchError::RejectedExecution; 3]);
  let dispatcher = dispatcher_with_executor(mailbox, runner, None, None);

  dispatcher.register_for_execution(register_user_hint());
  flaky.assert_attempts(3);
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
      crate::core::dispatch::mailbox::MailboxOverflowStrategy::DropNewest,
      None,
    )
    .with_throughput_limit(Some(NonZeroUsize::new(1).unwrap())),
  ));
  let system = ActorSystem::new_empty().state();
  let pid = system.allocate_pid();
  mailbox.set_instrumentation(MailboxInstrumentation::new(system.clone(), pid, None, None, None));

  let (tick, runner) = tick_executor_with_runner();
  let dispatcher = dispatcher_with_executor(mailbox.clone(), runner, Some(Duration::from_millis(1)), None);
  let invoker =
    MessageInvokerShared::new(Box::new(RecordingInvoker::default()) as Box<dyn MessageInvoker<NoStdToolbox>>);
  dispatcher.register_invoker(invoker);

  mailbox.enqueue_user(AnyMessage::new(1usize)).unwrap();
  mailbox.enqueue_user(AnyMessage::new(2usize)).unwrap();

  dispatcher.register_for_execution(register_user_hint());
  assert_eq!(tick.pending_tasks(), 1);
  tick.tick();
  assert!(tick.pending_tasks() <= 1);
}

#[test]
fn schedule_adapter_receives_pending_signal() {
  let (mailbox, _system) = bounded_mailbox(1);
  let (tick, runner) = tick_executor_with_runner();
  let adapter = ScheduleAdapterSharedGeneric::new(
    Box::new(CountingScheduleAdapter::default()) as Box<dyn ScheduleAdapter<NoStdToolbox>>
  );
  let dispatcher = dispatcher_with_executor_and_adapter(mailbox.clone(), runner, None, None, adapter.clone());
  let invoker =
    MessageInvokerShared::new(Box::new(RecordingInvoker::default()) as Box<dyn MessageInvoker<NoStdToolbox>>);
  dispatcher.register_invoker(invoker);

  mailbox.enqueue_user(AnyMessage::new(1usize)).expect("first message");
  let sender = dispatcher.into_sender();

  let handle = thread::spawn(move || {
    sender.send(AnyMessage::new(2usize)).expect("second message");
  });

  thread::sleep(Duration::from_millis(1));
  dispatcher.register_for_execution(register_user_hint());
  tick.tick();
  handle.join().expect("join");

  let pending_calls = adapter.with_write(|a| {
    a.as_any_mut().downcast_mut::<CountingScheduleAdapter>().expect("counting adapter").pending_calls()
  });

  assert!(pending_calls > 0);
}

#[test]
fn schedule_adapter_notified_on_rejection() {
  let (mailbox, system) = system_instrumented_mailbox();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(EventRecorder::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);

  let (flaky, runner) = flaky_executor_with_runner(vec![DispatchError::RejectedExecution; 3]);
  let adapter = ScheduleAdapterSharedGeneric::new(
    Box::new(CountingScheduleAdapter::default()) as Box<dyn ScheduleAdapter<NoStdToolbox>>
  );
  let dispatcher = dispatcher_with_executor_and_adapter(mailbox, runner, None, None, adapter.clone());

  dispatcher.register_for_execution(register_user_hint());
  flaky.assert_attempts(3);
  let rejected_calls = adapter.with_write(|a| {
    a.as_any_mut().downcast_mut::<CountingScheduleAdapter>().expect("counting adapter").rejected_calls()
  });

  assert!(rejected_calls >= 1);

  let logged = events.lock().iter().any(
    |event| matches!(event, crate::core::event_stream::EventStreamEvent::Log(log) if log.level() == LogLevel::Error),
  );
  assert!(logged, "expected rejection log entry");
}

#[test]
fn dispatcher_dump_event_published() {
  let (mailbox, system) = system_instrumented_mailbox();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(EventRecorder::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);

  let (_recording, runner) = recording_executor_with_runner();
  let adapter = ScheduleAdapterSharedGeneric::new(
    Box::new(CountingScheduleAdapter::default()) as Box<dyn ScheduleAdapter<NoStdToolbox>>
  );
  let dispatcher = dispatcher_with_executor_and_adapter(mailbox, runner, None, None, adapter);

  dispatcher.publish_dump_metrics();

  assert!(events.lock().iter().any(|event| matches!(event, EventStreamEvent::DispatcherDump(_))));
}

#[test]
fn telemetry_captures_mailbox_pressure_and_dispatcher_dump() {
  let (mailbox, system) = bounded_mailbox(2);
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(EventRecorder::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);

  let (_recording, runner) = recording_executor_with_runner();
  let adapter = ScheduleAdapterSharedGeneric::new(
    Box::new(CountingScheduleAdapter::default()) as Box<dyn ScheduleAdapter<NoStdToolbox>>
  );
  let dispatcher = dispatcher_with_executor_and_adapter(mailbox.clone(), runner, None, None, adapter);
  let invoker =
    MessageInvokerShared::new(Box::new(RecordingInvoker::default()) as Box<dyn MessageInvoker<NoStdToolbox>>);
  dispatcher.register_invoker(invoker);

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(1usize)), Ok(EnqueueOutcome::Enqueued)));
  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(2usize)), Ok(EnqueueOutcome::Enqueued)));
  dispatcher.publish_dump_metrics();

  let guard = events.lock();
  assert!(guard.iter().any(|event| matches!(event, EventStreamEvent::MailboxPressure(_))));
  assert!(guard.iter().any(|event| matches!(event, EventStreamEvent::DispatcherDump(_))));
}

fn dispatcher_with_executor(
  mailbox: ArcShared<MailboxGeneric<NoStdToolbox>>,
  executor: ArcShared<DispatchExecutorRunner<NoStdToolbox>>,
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
) -> DispatcherSharedGeneric<NoStdToolbox> {
  let adapter = crate::core::dispatch::dispatcher::InlineScheduleAdapter::shared::<NoStdToolbox>();
  dispatcher_with_executor_and_adapter(mailbox, executor, throughput_deadline, starvation_deadline, adapter)
}

fn dispatcher_with_executor_and_adapter(
  mailbox: ArcShared<MailboxGeneric<NoStdToolbox>>,
  executor: ArcShared<DispatchExecutorRunner<NoStdToolbox>>,
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
  adapter: ScheduleAdapterSharedGeneric<NoStdToolbox>,
) -> DispatcherSharedGeneric<NoStdToolbox> {
  DispatcherSharedGeneric::with_adapter(mailbox, executor, adapter, throughput_deadline, starvation_deadline)
}

fn recording_executor_with_runner() -> (ArcShared<RecordingExecutor>, ArcShared<DispatchExecutorRunner<NoStdToolbox>>) {
  let recording = ArcShared::new(RecordingExecutor::default());
  let recording_clone = recording.clone();
  let runner = ArcShared::new(DispatchExecutorRunner::new(Box::new(RecordingExecutorWrapper { inner: recording })));
  (recording_clone, runner)
}

fn flaky_executor_with_runner(
  failures: Vec<DispatchError>,
) -> (ArcShared<FlakyExecutor>, ArcShared<DispatchExecutorRunner<NoStdToolbox>>) {
  let flaky = ArcShared::new(FlakyExecutor::new(failures));
  let flaky_clone = flaky.clone();
  let runner = ArcShared::new(DispatchExecutorRunner::new(Box::new(FlakyExecutorWrapper { inner: flaky })));
  (flaky_clone, runner)
}

fn tick_executor_with_runner()
-> (ArcShared<TickExecutorGenericWrapper>, ArcShared<DispatchExecutorRunner<NoStdToolbox>>) {
  let tick = ArcShared::new(TickExecutorGenericWrapper::new());
  let tick_clone = tick.clone();
  let runner = ArcShared::new(DispatchExecutorRunner::new(Box::new(TickExecutorWrapper { inner: tick })));
  (tick_clone, runner)
}

struct RecordingExecutorWrapper {
  inner: ArcShared<RecordingExecutor>,
}

impl DispatchExecutor<NoStdToolbox> for RecordingExecutorWrapper {
  fn execute(&mut self, dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
    self.inner.calls.fetch_add(1, Ordering::Release);
    self.inner.tasks.lock().push(dispatcher);
    Ok(())
  }
}

struct FlakyExecutorWrapper {
  inner: ArcShared<FlakyExecutor>,
}

impl DispatchExecutor<NoStdToolbox> for FlakyExecutorWrapper {
  fn execute(&mut self, _dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
    self.inner.attempts.fetch_add(1, Ordering::Release);
    self.inner.failures.lock().pop().map_or_else(|| Ok(()), Err)
  }
}

struct TickExecutorWrapper {
  inner: ArcShared<TickExecutorGenericWrapper>,
}

struct TickExecutorGenericWrapper {
  executor: NoStdMutex<TickExecutorGeneric<NoStdToolbox>>,
}

impl TickExecutorGenericWrapper {
  fn new() -> Self {
    Self { executor: NoStdMutex::new(TickExecutorGeneric::new()) }
  }

  fn tick(&self) {
    self.executor.lock().tick();
  }

  fn pending_tasks(&self) -> usize {
    self.executor.lock().pending_tasks()
  }
}

impl DispatchExecutor<NoStdToolbox> for TickExecutorWrapper {
  fn execute(&mut self, dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
    self.inner.executor.lock().execute(dispatcher)
  }
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
  fn execute(&mut self, dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
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
  fn execute(&mut self, _dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
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
  fn invoke_user_message(&mut self, message: AnyMessage) -> Result<(), crate::core::error::ActorError> {
    if let Some(value) = message.payload().downcast_ref::<usize>() {
      self.messages.lock().push(*value);
    }
    Ok(())
  }

  fn invoke_system_message(
    &mut self,
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
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
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
  fn create_waker(&mut self, dispatcher: DispatcherSharedGeneric<NoStdToolbox>) -> core::task::Waker {
    ScheduleWaker::<NoStdToolbox>::into_waker(dispatcher)
  }

  fn on_pending(&mut self) {
    *self.pending.lock() += 1;
  }

  fn notify_rejected(&mut self, _attempts: usize) {
    *self.rejected.lock() += 1;
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}
