use alloc::{boxed::Box, vec::Vec};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system_with;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    messaging::{AnyMessage, system_message::SystemMessage},
    scheduler::SchedulerConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::StreamRefEndpointCleanup;
use crate::{
  StreamError,
  r#impl::streamref::stream_ref_endpoint_state::StreamRefEndpointState,
  stage::{StageActor, StageActorEnvelope, StageActorReceive},
  stream_ref::StreamRefRemoteStreamFailure,
};

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  create_noop_actor_system_with(|config| config.with_scheduler_config(scheduler))
}

struct NoopReceive;

impl StageActorReceive for NoopReceive {
  fn receive(&mut self, _envelope: StageActorEnvelope) -> Result<(), StreamError> {
    Ok(())
  }
}

struct RecordingSystemMessageSender {
  messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
}

impl RecordingSystemMessageSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, Self) {
    let messages = ArcShared::new(SpinSyncMutex::new(Vec::<SystemMessage>::new()));
    (messages.clone(), Self { messages })
  }
}

impl ActorRefSender for RecordingSystemMessageSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let system_message = message.downcast_ref::<SystemMessage>().expect("system message payload");
    self.messages.lock().push(system_message.clone());
    Ok(SendOutcome::Delivered)
  }
}

struct RecordingAnyMessageSender {
  messages: ArcShared<SpinSyncMutex<Vec<AnyMessage>>>,
}

impl RecordingAnyMessageSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<AnyMessage>>>, Self) {
    let messages = ArcShared::new(SpinSyncMutex::new(Vec::<AnyMessage>::new()));
    (messages.clone(), Self { messages })
  }
}

impl ActorRefSender for RecordingAnyMessageSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

struct FailingSystemMessageSender;

impl ActorRefSender for FailingSystemMessageSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::full(message))
  }
}

fn temp_actor_with_recording_sender(
  system: &ActorSystem,
  pid: Pid,
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>) {
  let (messages, sender) = RecordingSystemMessageSender::new();
  let actor = ActorRef::new(pid, ActorRefSenderShared::new(Box::new(sender)));
  let _name = system.state().register_temp_actor(actor.clone());
  (actor, messages)
}

fn temp_actor_with_recording_any_sender(
  system: &ActorSystem,
  pid: Pid,
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<AnyMessage>>>) {
  let (messages, sender) = RecordingAnyMessageSender::new();
  let actor = ActorRef::new(pid, ActorRefSenderShared::new(Box::new(sender)));
  let _name = system.state().register_temp_actor(actor.clone());
  (actor, messages)
}

fn temp_actor_with_failing_sender(system: &ActorSystem, pid: Pid) -> ActorRef {
  let actor = ActorRef::new(pid, ActorRefSenderShared::new(Box::new(FailingSystemMessageSender)));
  let _name = system.state().register_temp_actor(actor.clone());
  actor
}

#[test]
fn cleanup_unwatches_partner_and_stops_endpoint_actor() {
  let system = build_system();
  let partner_pid = system.allocate_pid();
  let (partner_actor, messages) = temp_actor_with_recording_sender(&system, partner_pid);
  let endpoint_actor = StageActor::new(&system, Box::new(NoopReceive));
  let endpoint_pid = endpoint_actor.actor_ref().pid();
  let mut state = StreamRefEndpointState::new();
  let cleanup = StreamRefEndpointCleanup { endpoint_actor, partner_actor: Some(partner_actor) };

  cleanup.run(&mut state).expect("cleanup");

  assert_eq!(*messages.lock(), vec![SystemMessage::Unwatch(endpoint_pid)]);
  assert_eq!(state.watch_release_failure(), None);
  assert_eq!(state.shutdown_failure(), None);
}

#[test]
fn cleanup_sends_cancellation_terminal_failure_to_partner() {
  let system = build_system();
  let partner_pid = system.allocate_pid();
  let (partner_actor, messages) = temp_actor_with_recording_any_sender(&system, partner_pid);
  let endpoint_actor = StageActor::new(&system, Box::new(NoopReceive));
  let endpoint_pid = endpoint_actor.actor_ref().pid();
  let mut state = StreamRefEndpointState::new();
  state.cancel();
  let cleanup = StreamRefEndpointCleanup { endpoint_actor, partner_actor: Some(partner_actor) };

  cleanup.run(&mut state).expect("cleanup");

  let messages = messages.lock();
  assert_eq!(messages.len(), 2);
  let signal = messages[0].downcast_ref::<StreamRefRemoteStreamFailure>().expect("remote stream failure signal");
  assert_eq!(signal.message(), "NoMoreElementsNeeded");
  assert_eq!(messages[0].sender().map(|sender| sender.pid()), Some(endpoint_pid));
  assert_eq!(messages[1].downcast_ref::<SystemMessage>(), Some(&SystemMessage::Unwatch(endpoint_pid)));
}

#[test]
fn cleanup_records_watch_release_failure() {
  let system = build_system();
  let partner_pid = system.allocate_pid();
  let partner_actor = temp_actor_with_failing_sender(&system, partner_pid);
  let endpoint_actor = StageActor::new(&system, Box::new(NoopReceive));
  let mut state = StreamRefEndpointState::new();
  let cleanup = StreamRefEndpointCleanup { endpoint_actor, partner_actor: Some(partner_actor) };

  let error = cleanup.run(&mut state).expect_err("watch release failure");

  assert_eq!(error, StreamError::WouldBlock);
  assert_eq!(state.watch_release_failure(), Some(&StreamError::WouldBlock));
  assert_eq!(state.shutdown_failure(), None);
}

#[test]
fn cleanup_records_shutdown_failure() {
  let system = build_system();
  let watcher_pid = system.allocate_pid();
  let _watcher = temp_actor_with_failing_sender(&system, watcher_pid);
  let endpoint_actor = StageActor::new(&system, Box::new(NoopReceive));
  system
    .state()
    .send_system_message(endpoint_actor.actor_ref().pid(), SystemMessage::Watch(watcher_pid))
    .expect("register watcher");
  let mut state = StreamRefEndpointState::new();
  let cleanup = StreamRefEndpointCleanup { endpoint_actor, partner_actor: None };

  let error = cleanup.run(&mut state).expect_err("shutdown failure");

  assert_eq!(error, StreamError::WouldBlock);
  assert_eq!(state.watch_release_failure(), None);
  assert_eq!(state.shutdown_failure(), Some(&StreamError::WouldBlock));
}

#[test]
fn cleanup_combines_watch_release_and_shutdown_failures() {
  let system = build_system();
  let partner_pid = system.allocate_pid();
  let watcher_pid = system.allocate_pid();
  let partner_actor = temp_actor_with_failing_sender(&system, partner_pid);
  let _watcher = temp_actor_with_failing_sender(&system, watcher_pid);
  let endpoint_actor = StageActor::new(&system, Box::new(NoopReceive));
  system
    .state()
    .send_system_message(endpoint_actor.actor_ref().pid(), SystemMessage::Watch(watcher_pid))
    .expect("register watcher");
  let mut state = StreamRefEndpointState::new();
  let cleanup = StreamRefEndpointCleanup { endpoint_actor, partner_actor: Some(partner_actor) };

  let error = cleanup.run(&mut state).expect_err("combined cleanup failure");

  assert!(matches!(error, StreamError::MaterializedResourceRollbackFailed { .. }));
  assert_eq!(error.materialization_primary_failure(), Some(&StreamError::WouldBlock));
  assert_eq!(error.materialization_cleanup_failure(), Some(&StreamError::WouldBlock));
  assert_eq!(state.watch_release_failure(), Some(&StreamError::WouldBlock));
  assert_eq!(state.shutdown_failure(), Some(&StreamError::WouldBlock));
}
