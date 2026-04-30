extern crate std;

use fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system;
use fraktor_actor_core_rs::core::kernel::actor::{Actor, ActorContext, Pid, error::ActorError, messaging::AnyMessage};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::super::{Stream, StreamIslandActor, StreamIslandCommand, StreamIslandDriveGate, StreamShared, StreamState};
use crate::core::{
  DynValue, SourceLogic, StreamError,
  dsl::{Sink, Source},
  r#impl::fusing::StreamBufferConfig,
  materialization::{KeepRight, StreamNotUsed},
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
}

impl SourceLogic for CancelFailingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(None)
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

fn new_stream_island_actor(stream: StreamShared) -> StreamIslandActor {
  StreamIslandActor::new(stream, new_drive_gate())
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
fn shutdown_command_returns_error_when_cancel_fails() {
  let stream = cancel_failing_stream();
  let mut actor = new_stream_island_actor(stream);

  let result = receive_command_result(&mut actor, StreamIslandCommand::Shutdown);

  assert!(result.is_err());
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
fn drive_gate_rejects_second_pending_drive_until_idle() {
  let gate = new_drive_gate();

  assert!(gate.try_mark_pending());
  assert!(!gate.try_mark_pending());

  gate.mark_idle();

  assert!(gate.try_mark_pending());
}

#[test]
fn drive_command_releases_pending_gate_after_processing() {
  let pulls = new_pull_counter();
  let stream = counting_stream(pulls);
  let gate = new_drive_gate();
  let mut actor = StreamIslandActor::new(stream, gate.clone());
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
  let mut actor = StreamIslandActor::new(stream, gate.clone());
  assert!(gate.try_mark_pending());

  receive_command(&mut actor, StreamIslandCommand::Drive);

  assert!(gate.try_mark_pending());
}
