use core::{marker::PhantomData, num::NonZeroU64};

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

use super::{
  ActorBackedSinkRefLogic, ActorBackedSinkRefReceive, ActorBackedSinkRefStateShared, SinkRef, SinkRefBackend,
};
use crate::{
  DemandTracker, DynValue, StreamError,
  dsl::Sink,
  r#impl::streamref::{StreamRefEndpointSlot, StreamRefHandoff},
  materialization::StreamNotUsed,
  sink_logic::SinkLogic,
  stage::{StageActor, StageActorEnvelope, StageActorReceive},
  stream_ref::{StreamRefAck, StreamRefCumulativeDemand, StreamRefRemoteStreamFailure},
};

struct RecordingSender {
  system_messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
  user_messages:   ArcShared<SpinSyncMutex<usize>>,
}

struct UserFailingSender {
  system_messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
}

struct SystemFailingSender {
  user_messages: ArcShared<SpinSyncMutex<usize>>,
}

impl RecordingSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<usize>>, Self) {
    let system_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
    let user_messages = ArcShared::new(SpinSyncMutex::new(0_usize));
    let sender = Self { system_messages: system_messages.clone(), user_messages: user_messages.clone() };
    (system_messages, user_messages, sender)
  }
}

impl UserFailingSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, Self) {
    let system_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
    let sender = Self { system_messages: system_messages.clone() };
    (system_messages, sender)
  }
}

impl SystemFailingSender {
  fn new() -> (ArcShared<SpinSyncMutex<usize>>, Self) {
    let user_messages = ArcShared::new(SpinSyncMutex::new(0_usize));
    let sender = Self { user_messages: user_messages.clone() };
    (user_messages, sender)
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(system_message) = message.downcast_ref::<SystemMessage>() {
      self.system_messages.lock().push(system_message.clone());
    } else {
      *self.user_messages.lock() += 1;
    }
    Ok(SendOutcome::Delivered)
  }
}

impl ActorRefSender for UserFailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(system_message) = message.downcast_ref::<SystemMessage>() {
      self.system_messages.lock().push(system_message.clone());
      return Ok(SendOutcome::Delivered);
    }
    Err(SendError::closed(message))
  }
}

impl ActorRefSender for SystemFailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if message.downcast_ref::<SystemMessage>().is_some() {
      return Err(SendError::closed(message));
    }
    *self.user_messages.lock() += 1;
    Ok(SendOutcome::Delivered)
  }
}

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  create_noop_actor_system_with(|config| config.with_scheduler_config(scheduler))
}

fn temp_recording_actor(
  system: &ActorSystem,
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<usize>>) {
  let (system_messages, user_messages, sender) = RecordingSender::new();
  let system_state = system.state();
  let actor_ref =
    ActorRef::from_shared(system.allocate_pid(), ActorRefSenderShared::new(Box::new(sender)), &system_state);
  let _name = system_state.register_temp_actor(actor_ref.clone());
  (actor_ref, system_messages, user_messages)
}

fn temp_user_failing_actor(system: &ActorSystem) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>) {
  let (system_messages, sender) = UserFailingSender::new();
  let system_state = system.state();
  let actor_ref =
    ActorRef::from_shared(system.allocate_pid(), ActorRefSenderShared::new(Box::new(sender)), &system_state);
  let _name = system_state.register_temp_actor(actor_ref.clone());
  (actor_ref, system_messages)
}

fn temp_system_failing_actor(system: &ActorSystem) -> (ActorRef, ArcShared<SpinSyncMutex<usize>>) {
  let (user_messages, sender) = SystemFailingSender::new();
  let system_state = system.state();
  let actor_ref =
    ActorRef::from_shared(system.allocate_pid(), ActorRefSenderShared::new(Box::new(sender)), &system_state);
  let _name = system_state.register_temp_actor(actor_ref.clone());
  (actor_ref, user_messages)
}

#[test]
fn into_sink_consumes_sink_ref() {
  let handoff = StreamRefHandoff::<u32>::new();
  let sink_ref = SinkRef::new(handoff, StreamRefEndpointSlot::new());

  let _sink: Sink<u32, StreamNotUsed> = sink_ref.into_sink();
}

#[test]
fn into_sink_accepts_actor_backed_and_failed_endpoint_variants() {
  let system = build_system();
  let (endpoint_actor, _system_messages, _user_messages) = temp_recording_actor(&system);
  let sink_ref = SinkRef::<u32>::from_endpoint_actor(endpoint_actor);

  let _actor_backed_sink: Sink<u32, StreamNotUsed> = sink_ref.into_sink();

  let failed_ref = SinkRef::<u32> {
    backend: SinkRefBackend::ActorBacked { endpoint: StreamRefEndpointSlot::new() },
    _pd:     PhantomData,
  };
  let _failed_sink: Sink<u32, StreamNotUsed> = failed_ref.into_sink();
}

#[test]
fn actor_backed_sink_ref_reports_startup_and_missing_endpoint_errors() {
  let mut failed = ActorBackedSinkRefLogic::<u32>::failed(StreamError::Failed);
  let mut demand = DemandTracker::new();

  assert!(!failed.can_accept_input());
  assert_eq!(failed.on_start(&mut demand), Err(StreamError::Failed));
  let input: DynValue = Box::new(10_u32);
  assert_eq!(failed.on_push(input, &mut demand), Err(StreamError::Failed));
  assert_eq!(failed.on_tick(&mut demand), Err(StreamError::Failed));
  failed.attach_actor_system(build_system());

  let mut logic = ActorBackedSinkRefLogic::<u32>::new(ActorRef::null());
  let helper_error = ActorBackedSinkRefLogic::<u32>::stream_error_from_context("boom");
  assert!(matches!(helper_error, StreamError::FailedWithContext { .. }));
  assert_eq!(logic.drain_endpoint_actor(), Ok(()));
  assert_eq!(logic.watch_target_actor(), Err(StreamError::StreamRefTargetNotInitialized));
  assert_eq!(logic.release_target_watch(), Ok(()));
  assert_eq!(logic.serialize_value(&10_u32), Err(StreamError::StreamRefTargetNotInitialized));

  logic.set_startup_result(Err(StreamError::Failed));
  assert_eq!(logic.startup_error, Some(StreamError::Failed));
}

#[test]
fn actor_backed_sink_ref_attach_stops_after_prior_startup_error() {
  let system = build_system();
  let (target, system_messages, user_messages) = temp_recording_actor(&system);
  let mut logic = ActorBackedSinkRefLogic::<u32>::new(target);
  logic.startup_error = Some(StreamError::Failed);

  logic.attach_actor_system(system);

  let endpoint_pid = logic.endpoint_actor_ref().expect("endpoint actor ref").pid();
  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid)]);
  assert_eq!(*user_messages.lock(), 0);
}

#[test]
fn actor_backed_sink_ref_on_error_sends_failure_and_records_send_errors() {
  let system = build_system();
  let (target, system_messages, user_messages) = temp_recording_actor(&system);
  let mut logic = ActorBackedSinkRefLogic::<u32>::new(target);
  logic.attach_actor_system(system.clone());
  let endpoint_pid = logic.endpoint_actor_ref().expect("endpoint actor ref").pid();

  logic.on_error(StreamError::Failed);

  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid), SystemMessage::Unwatch(endpoint_pid)]);
  assert_eq!(*user_messages.lock(), 2);

  let mut send_failed = ActorBackedSinkRefLogic::<u32>::failed(StreamError::Failed);
  send_failed.on_error(StreamError::StreamDetached);
  assert_eq!(send_failed.state.error_result(), Err(StreamError::StreamRefTargetNotInitialized));

  let endpoint_actor = StageActor::new(
    &system,
    Box::new(ActorBackedSinkRefReceive::new(
      ActorBackedSinkRefStateShared::new(),
      Err(StreamError::StreamRefTargetNotInitialized),
    )),
  );
  let mut release_failed = ActorBackedSinkRefLogic::<u32>::failed(StreamError::Failed);
  release_failed.endpoint_actor = Some(endpoint_actor);
  release_failed.on_error(StreamError::StreamDetached);
  match release_failed.state.error_result() {
    | Err(StreamError::FailedWithContext { message, .. }) => {
      assert!(message.contains("failed to notify StreamRef target"));
      assert!(message.contains("failed to release target watch"));
    },
    | other => panic!("expected combined send/release failure, got {other:?}"),
  }
}

#[test]
fn actor_backed_sink_ref_on_error_records_release_failure_after_notify_success() {
  let system = build_system();
  let (target, user_messages) = temp_system_failing_actor(&system);
  let mut logic = ActorBackedSinkRefLogic::<u32>::new(target);
  logic.attach_actor_system(system);

  logic.on_error(StreamError::Failed);

  assert_eq!(*user_messages.lock(), 1);
  assert!(matches!(logic.state.error_result(), Err(StreamError::FailedWithContext { .. })));
}

#[test]
fn actor_backed_sink_ref_watches_target_and_releases_on_complete() {
  let system = build_system();
  let (target, system_messages, user_messages) = temp_recording_actor(&system);
  let mut logic = ActorBackedSinkRefLogic::<u32>::new(target);

  logic.attach_actor_system(system);
  let endpoint_pid = logic.endpoint_actor_ref().expect("endpoint actor ref").pid();
  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid)]);
  assert_eq!(*user_messages.lock(), 1);

  logic.on_complete().expect("complete");

  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid), SystemMessage::Unwatch(endpoint_pid)]);
  assert_eq!(*user_messages.lock(), 2);
}

#[test]
fn actor_backed_sink_ref_releases_watch_when_completion_send_fails() {
  let system = build_system();
  let (target, system_messages) = temp_user_failing_actor(&system);
  let mut logic = ActorBackedSinkRefLogic::<u32>::new(target);

  logic.attach_actor_system(system);
  let endpoint_pid = logic.endpoint_actor_ref().expect("endpoint actor ref").pid();
  let error = logic.on_complete().expect_err("completion send should fail");

  assert!(matches!(error, StreamError::FailedWithContext { .. }));
  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid), SystemMessage::Unwatch(endpoint_pid)]);
}

#[test]
fn actor_backed_sink_ref_state_ignores_stale_cumulative_demand() {
  let state = ActorBackedSinkRefStateShared::new();
  let demand = NonZeroU64::new(1).expect("demand");
  state.subscribe();

  assert_eq!(state.accept_demand(0, demand), Ok(()));
  assert_eq!(state.accept_demand(0, demand), Ok(()));
  assert_eq!(state.reserve_next_seq_nr(), Ok(0));
  assert_eq!(state.accept_demand(0, demand), Ok(()));
  assert_eq!(state.reserve_next_seq_nr(), Err(StreamError::WouldBlock));
  assert_eq!(state.accept_demand(1, demand), Ok(()));
  assert_eq!(state.reserve_next_seq_nr(), Ok(1));
}

#[test]
fn actor_backed_sink_ref_state_reports_failure_and_sequence_errors() {
  let state = ActorBackedSinkRefStateShared::new();
  let demand = NonZeroU64::new(1).expect("demand");

  assert!(!state.can_accept_input());
  assert_eq!(state.error_result(), Ok(()));
  assert_eq!(state.reserve_next_seq_nr(), Err(StreamError::WouldBlock));
  state.subscribe();
  assert!(matches!(state.accept_demand(1, demand), Err(StreamError::InvalidSequenceNumber { .. })));
  assert_eq!(state.accept_demand(0, demand), Ok(()));
  assert!(state.can_accept_input());
  state.fail(StreamError::Failed);
  assert_eq!(state.error_result(), Err(StreamError::Failed));
  assert!(!state.can_accept_input());
  assert_eq!(state.reserve_next_seq_nr(), Err(StreamError::Failed));
  assert_eq!(state.accept_demand(0, demand), Err(StreamError::Failed));
}

#[test]
fn actor_backed_sink_ref_receive_accepts_partner_protocols() {
  let system = build_system();
  let (target, _system_messages, _user_messages) = temp_recording_actor(&system);
  let sender_key = target.canonical_path().expect("canonical path").to_canonical_uri();
  let state = ActorBackedSinkRefStateShared::new();
  let mut receive = ActorBackedSinkRefReceive::new(state.clone(), Ok(sender_key));
  let demand = NonZeroU64::new(2).expect("demand");

  receive.receive(StageActorEnvelope::new(target.clone(), AnyMessage::new(StreamRefAck))).expect("ack accepted");
  receive
    .receive(StageActorEnvelope::new(target.clone(), AnyMessage::new(StreamRefCumulativeDemand::new(0, demand))))
    .expect("demand accepted");

  assert!(state.can_accept_input());

  receive
    .receive(StageActorEnvelope::new(target, AnyMessage::new(StreamRefRemoteStreamFailure::new(String::from("boom")))))
    .expect("failure accepted");
  assert!(matches!(state.error_result(), Err(StreamError::FailedWithContext { .. })));
}

#[test]
fn actor_backed_sink_ref_receive_rejects_invalid_sender_unknown_message_and_deathwatch() {
  let system = build_system();
  let (target, _target_system_messages, _target_user_messages) = temp_recording_actor(&system);
  let (other, _other_system_messages, _other_user_messages) = temp_recording_actor(&system);
  let sender_key = target.canonical_path().expect("canonical path").to_canonical_uri();
  let mut receive = ActorBackedSinkRefReceive::new(ActorBackedSinkRefStateShared::new(), Ok(sender_key));

  let invalid_sender_error =
    receive.receive(StageActorEnvelope::new(other, AnyMessage::new(StreamRefAck))).expect_err("invalid sender");
  let unknown_message_error =
    receive.receive(StageActorEnvelope::new(target.clone(), AnyMessage::new(7_u32))).expect_err("unknown message");
  let deathwatch_error = receive
    .receive(StageActorEnvelope::new(target, AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(99, 0)))))
    .expect_err("deathwatch");

  assert!(matches!(invalid_sender_error, StreamError::InvalidPartnerActor { .. }));
  assert_eq!(unknown_message_error, StreamError::Failed);
  assert!(matches!(deathwatch_error, StreamError::RemoteStreamRefActorTerminated { .. }));
}
