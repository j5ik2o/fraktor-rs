extern crate std;

use fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system;
use fraktor_actor_core_rs::core::kernel::actor::{
  Actor, ActorContext, Pid, error::ActorError, messaging::AnyMessage, scheduler::SchedulerHandle,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::super::{Stream, StreamIslandActor, StreamIslandCommand, StreamIslandDriveGate, StreamShared, StreamState};
use crate::core::{
  DynValue, SourceLogic, StreamError,
  dsl::{Sink, Source},
  r#impl::fusing::StreamBufferConfig,
  materialization::{
    DownstreamCancellationControlPlaneShared, DriveOutcome, KeepRight, StreamNotUsed,
    empty_downstream_cancellation_control_plane,
  },
  stage::StageKind,
};

type PullCounter = ArcShared<SpinSyncMutex<u32>>;

struct CountingSourceLogic {
  pulls: PullCounter,
  next:  Option<u32>,
}

struct CancelFailingSourceLogic;

impl CountingSourceLogic {
  fn new(pulls: PullCounter) -> Self {
    Self { pulls, next: Some(1) }
  }
}

impl SourceLogic for CountingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    *self.pulls.lock() += 1;
    Ok(self.next.take().map(|value| Box::new(value) as DynValue))
  }

  fn should_drain_on_shutdown(&self) -> bool {
    false
  }
}

impl SourceLogic for CancelFailingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(None)
  }

  fn should_drain_on_shutdown(&self) -> bool {
    false
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    Err(StreamError::Failed)
  }
}

fn new_pull_counter() -> PullCounter {
  ArcShared::new(SpinSyncMutex::new(0))
}

fn pull_count(pulls: &PullCounter) -> u32 {
  *pulls.lock()
}

fn counting_stream(pulls: PullCounter) -> StreamShared {
  let graph = Source::<u32, StreamNotUsed>::from_logic(StageKind::Custom, CountingSourceLogic::new(pulls))
    .into_mat(Sink::ignore(), KeepRight);
  let (plan, _completion) = graph.into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("stream should start");
  StreamShared::new(stream)
}

fn new_drive_gate() -> StreamIslandDriveGate {
  StreamIslandDriveGate::new()
}

fn new_downstream_cancellation_control_plane() -> DownstreamCancellationControlPlaneShared {
  empty_downstream_cancellation_control_plane()
}

fn new_tick_handle_slot() -> ArcShared<SpinSyncMutex<Option<SchedulerHandle>>> {
  ArcShared::new(SpinSyncMutex::new(None))
}

fn new_stream_island_actor(stream: StreamShared) -> StreamIslandActor {
  StreamIslandActor::new(
    stream.clone(),
    new_drive_gate(),
    new_downstream_cancellation_control_plane(),
    vec![stream],
    new_tick_handle_slot(),
  )
}

fn cancel_failing_stream() -> StreamShared {
  let graph = Source::<u32, StreamNotUsed>::from_logic(StageKind::SourceSingle, CancelFailingSourceLogic)
    .into_mat(Sink::ignore(), KeepRight);
  let (plan, _completion) = graph.into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("stream should start");
  StreamShared::new(stream)
}

fn receive_command_result(actor: &mut StreamIslandActor, command: StreamIslandCommand) -> Result<(), ActorError> {
  let system = new_empty_actor_system();
  let pid = Pid::new(1, 1);
  let mut context = ActorContext::new(&system, pid);
  let message = AnyMessage::new(command);

  actor.receive(&mut context, message.as_view())
}

fn receive_command(actor: &mut StreamIslandActor, command: StreamIslandCommand) {
  receive_command_result(actor, command).expect("receive failed");
}

fn receive_message_result(actor: &mut StreamIslandActor, message: AnyMessage) -> Result<(), ActorError> {
  let system = new_empty_actor_system();
  let pid = Pid::new(1, 1);
  let mut context = ActorContext::new(&system, pid);

  actor.receive(&mut context, message.as_view())
}

#[test]
fn drive_command_drives_owned_stream_from_mailbox() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls.clone());
  let mut actor = new_stream_island_actor(stream);
  let pulls_before = pull_count(&pulls);

  receive_command(&mut actor, StreamIslandCommand::Drive);

  assert!(pull_count(&pulls) > pulls_before);
}

#[test]
fn drive_command_does_not_drive_terminal_stream() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls.clone());
  stream.cancel().expect("cancel should reach terminal state");
  assert_eq!(stream.state(), StreamState::Cancelled);
  let mut actor = new_stream_island_actor(stream);
  let pulls_before = pull_count(&pulls);

  receive_command(&mut actor, StreamIslandCommand::Drive);

  assert_eq!(pull_count(&pulls), pulls_before);
}

#[test]
fn cancel_command_cancels_owned_stream() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  let observed = stream.clone();
  let mut actor = new_stream_island_actor(stream);

  receive_command(&mut actor, StreamIslandCommand::Cancel { cause: None });

  assert_eq!(observed.state(), StreamState::Cancelled);
}

#[test]
fn shutdown_command_completes_owned_stream_gracefully() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  let observed = stream.clone();
  let mut actor = new_stream_island_actor(stream);

  receive_command(&mut actor, StreamIslandCommand::Shutdown);

  assert_eq!(observed.state(), StreamState::Completed);
}

#[test]
fn cancel_command_returns_error_when_cancel_fails() {
  let stream = cancel_failing_stream();
  let mut actor = new_stream_island_actor(stream);

  let result = receive_command_result(&mut actor, StreamIslandCommand::Cancel { cause: None });

  assert!(result.is_err());
}

#[test]
fn cancel_command_reports_cause_when_cancel_fails_after_abort() {
  let stream = cancel_failing_stream();
  let mut actor = new_stream_island_actor(stream);

  let result = receive_command_result(&mut actor, StreamIslandCommand::Cancel { cause: Some(StreamError::Failed) });

  assert!(result.is_err());
}

#[test]
fn shutdown_command_returns_error_when_cancel_fails() {
  let stream = cancel_failing_stream();
  let mut actor = new_stream_island_actor(stream);

  let result = receive_command_result(&mut actor, StreamIslandCommand::Shutdown);

  assert!(result.is_err());
}

#[test]
fn drive_fails_stream_when_kill_switch_shutdown_request_fails() {
  let graph = Source::<u32, StreamNotUsed>::from_logic(StageKind::SourceSingle, CancelFailingSourceLogic)
    .into_mat(Sink::ignore(), KeepRight);
  let (plan, _completion) = graph.into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("stream should start");
  let kill_switch_state = stream.kill_switch_state();
  assert!(kill_switch_state.lock().request_shutdown().is_some());

  let outcome = stream.drive();

  assert_eq!(outcome, DriveOutcome::Progressed);
  assert_eq!(stream.state(), StreamState::Failed);
}

#[test]
fn drive_aborts_stream_when_graph_kill_switch_is_aborted() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  let kill_switch_state = stream.with_read(|stream| stream.kill_switch_state());
  assert!(kill_switch_state.lock().request_abort(StreamError::Failed).is_some());

  let outcome = stream.drive();

  assert_eq!(outcome, DriveOutcome::Progressed);
  assert_eq!(stream.state(), StreamState::Failed);
}

#[test]
fn abort_command_fails_owned_stream_and_returns_error() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  let observed = stream.clone();
  let mut actor = new_stream_island_actor(stream);

  let result = receive_command_result(&mut actor, StreamIslandCommand::Abort(StreamError::Failed));

  assert!(result.is_err());
  assert_eq!(observed.state(), StreamState::Failed);
}

#[test]
fn abort_graph_streams_marks_graph_kill_switch_aborted() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  let kill_switch_state = stream.with_read(|stream| stream.kill_switch_state());
  let actor = new_stream_island_actor(stream.clone());

  actor.abort_graph_streams(&StreamError::Failed);

  assert_eq!(kill_switch_state.lock().abort_error(), Some(StreamError::Failed));
  assert_eq!(stream.state(), StreamState::Failed);
}

#[test]
fn non_stream_island_command_is_ignored() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  let observed = stream.clone();
  let mut actor = new_stream_island_actor(stream);

  let result = receive_message_result(&mut actor, AnyMessage::new(123_u32));

  assert_eq!(result, Ok(()));
  assert_eq!(observed.state(), StreamState::Running);
}

#[test]
fn drive_command_releases_pending_gate_after_processing() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  let gate = new_drive_gate();
  let mut actor = StreamIslandActor::new(
    stream.clone(),
    gate.clone(),
    new_downstream_cancellation_control_plane(),
    vec![stream],
    new_tick_handle_slot(),
  );
  assert!(gate.try_mark_pending());

  receive_command(&mut actor, StreamIslandCommand::Drive);

  assert!(gate.try_mark_pending());
}

#[test]
fn drive_command_releases_pending_gate_when_stream_is_terminal() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  stream.cancel().expect("cancel should reach terminal state");
  let gate = new_drive_gate();
  let mut actor = StreamIslandActor::new(
    stream.clone(),
    gate.clone(),
    new_downstream_cancellation_control_plane(),
    vec![stream],
    new_tick_handle_slot(),
  );
  assert!(gate.try_mark_pending());

  receive_command(&mut actor, StreamIslandCommand::Drive);

  assert!(gate.try_mark_pending());
}
