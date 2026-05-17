use core::marker::PhantomData;

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system_with;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    messaging::{AnyMessage, system_message::SystemMessage},
    scheduler::SchedulerConfig,
  },
  serialization::{SerializedMessage, SerializerId},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::{ActorBackedSourceRefLogic, ActorBackedSourceRefReceive, SourceRef, SourceRefBackend};
use crate::{
  StreamError,
  dsl::Source,
  r#impl::streamref::{StreamRefEndpointSlot, StreamRefHandoff},
  materialization::StreamNotUsed,
  source_logic::SourceLogic,
  stage::{StageActorEnvelope, StageActorReceive},
  stream_ref::{
    StreamRefAck, StreamRefRemoteStreamCompleted, StreamRefRemoteStreamFailure, StreamRefSequencedOnNext,
    StreamRefSettings,
  },
};

struct RecordingSender {
  system_messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
  user_messages:   ArcShared<SpinSyncMutex<usize>>,
}

impl RecordingSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<usize>>, Self) {
    let system_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
    let user_messages = ArcShared::new(SpinSyncMutex::new(0_usize));
    let sender = Self { system_messages: system_messages.clone(), user_messages: user_messages.clone() };
    (system_messages, user_messages, sender)
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

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  create_noop_actor_system_with(|config| config.with_scheduler_config(scheduler))
}

fn temp_recording_actor(
  system: &ActorSystem,
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<usize>>) {
  let pid = system.allocate_pid();
  temp_recording_actor_with_pid(system, pid)
}

fn temp_recording_actor_with_pid(
  system: &ActorSystem,
  pid: Pid,
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<usize>>) {
  let (system_messages, user_messages, sender) = RecordingSender::new();
  let system_state = system.state();
  let actor_ref = ActorRef::from_shared(pid, ActorRefSenderShared::new(Box::new(sender)), &system_state);
  let _name = system_state.register_temp_actor(actor_ref.clone());
  (actor_ref, system_messages, user_messages)
}

#[test]
fn into_source_consumes_source_ref() {
  let handoff = StreamRefHandoff::<u32>::new();
  let source_ref = SourceRef::new(handoff, StreamRefEndpointSlot::new());

  let _source: Source<u32, StreamNotUsed> = source_ref.into_source();
}

#[test]
fn into_source_accepts_actor_backed_and_failed_endpoint_variants() {
  let system = build_system();
  let (target, _system_messages, _user_messages) = temp_recording_actor(&system);
  let source_ref = SourceRef::<u32>::from_endpoint_actor(target);

  let _actor_backed_source: Source<u32, StreamNotUsed> = source_ref.into_source();

  let failed_ref = SourceRef::<u32> {
    backend: SourceRefBackend::ActorBacked { endpoint: StreamRefEndpointSlot::new() },
    _pd:     PhantomData,
  };
  let _failed_source: Source<u32, StreamNotUsed> = failed_ref.into_source();
}

#[test]
fn actor_backed_source_ref_watches_target_when_attached() {
  let system = build_system();
  let (target, system_messages, user_messages) = temp_recording_actor(&system);
  let mut logic = ActorBackedSourceRefLogic::<u32>::new(target);

  logic.attach_actor_system(system);

  let endpoint_pid = logic.endpoint_actor_ref().expect("endpoint actor ref").pid();
  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid)]);
  assert_eq!(*user_messages.lock(), 1);
}

#[test]
fn actor_backed_source_ref_fails_when_partner_never_subscribes_before_timeout() {
  let system = build_system();
  let (target, _system_messages, _user_messages) = temp_recording_actor(&system);
  let mut logic = ActorBackedSourceRefLogic::<u32>::new(target);
  logic.attach_stream_ref_settings(StreamRefSettings::new().with_subscription_timeout_ticks(1));

  let error = logic.pull().expect_err("subscription timeout");

  assert!(matches!(error, StreamError::StreamRefSubscriptionTimeout { .. }));
}

#[test]
fn actor_backed_source_ref_reports_startup_and_missing_endpoint_errors() {
  let mut logic = ActorBackedSourceRefLogic::<u32>::new(ActorRef::null());

  assert_eq!(logic.drain_endpoint_actor(), Ok(()));
  assert_eq!(logic.watch_target_actor(), Err(StreamError::StreamRefTargetNotInitialized));
  logic.set_startup_result(Err(StreamError::Failed));
  assert_eq!(logic.startup_error, Some(StreamError::Failed));
  assert_eq!(logic.pull().expect_err("startup error"), StreamError::Failed);
  assert_eq!(logic.on_cancel(), Ok(()));
  assert!(!logic.should_drain_on_shutdown());

  let helper_error = ActorBackedSourceRefReceive::<u32>::stream_error_from_context("boom");
  assert!(matches!(helper_error, StreamError::FailedWithContext { .. }));
}

#[test]
fn actor_backed_source_ref_waits_before_timeout_and_reports_handoff_failure() {
  let mut waiting = ActorBackedSourceRefLogic::<u32>::new(ActorRef::null());
  waiting.attach_stream_ref_settings(StreamRefSettings::new().with_subscription_timeout_ticks(2));

  assert_eq!(waiting.pull().expect_err("waiting should block"), StreamError::WouldBlock);

  let mut failed = ActorBackedSourceRefLogic::<u32>::new(ActorRef::null());
  failed.handoff.subscribe();
  failed.handoff.fail(StreamError::Failed);

  assert_eq!(failed.pull().expect_err("handoff failure"), StreamError::Failed);
}

#[test]
fn actor_backed_source_ref_attach_stops_after_prior_startup_error() {
  let system = build_system();
  let (target, system_messages, user_messages) = temp_recording_actor(&system);
  let mut logic = ActorBackedSourceRefLogic::<u32>::new(target);
  logic.startup_error = Some(StreamError::Failed);

  logic.attach_actor_system(system);

  let endpoint_pid = logic.endpoint_actor_ref().expect("endpoint actor ref").pid();
  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid)]);
  assert_eq!(*user_messages.lock(), 0);
}

#[test]
fn actor_backed_source_ref_sends_demand_after_subscription() {
  let system = build_system();
  let (target, _system_messages, user_messages) = temp_recording_actor(&system);
  let mut logic = ActorBackedSourceRefLogic::<u32>::new(target);
  logic.attach_actor_system(system);
  logic.handoff.subscribe();

  let error = logic.pull().expect_err("empty remote source should block after demand");

  assert_eq!(error, StreamError::WouldBlock);
  assert_eq!(*user_messages.lock(), 2);
}

#[test]
fn actor_backed_source_ref_receive_accepts_ack_completion_and_failure() {
  let system = build_system();
  let (target, _system_messages, _user_messages) = temp_recording_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = ActorBackedSourceRefReceive::new(handoff.clone(), system, &target);

  receive.receive(StageActorEnvelope::new(target.clone(), AnyMessage::new(StreamRefAck))).expect("ack accepted");
  assert!(handoff.is_subscribed());

  receive
    .receive(StageActorEnvelope::new(target.clone(), AnyMessage::new(StreamRefRemoteStreamCompleted::new(0))))
    .expect("completion accepted");
  assert_eq!(handoff.poll_or_drain(), Ok(None));

  let failed = StreamRefHandoff::<u32>::new();
  let mut failed_receive = ActorBackedSourceRefReceive::new(failed.clone(), build_system(), &target);
  failed_receive
    .receive(StageActorEnvelope::new(target, AnyMessage::new(StreamRefRemoteStreamFailure::new(String::from("boom")))))
    .expect("failure accepted");
  assert!(matches!(failed.poll_or_drain(), Err(StreamError::FailedWithContext { .. })));
}

#[test]
fn actor_backed_source_ref_receive_rejects_invalid_sender() {
  let system = build_system();
  let (target, _target_system_messages, _target_user_messages) = temp_recording_actor(&system);
  let (other, _other_system_messages, _other_user_messages) = temp_recording_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = ActorBackedSourceRefReceive::new(handoff.clone(), system, &target);

  let error =
    receive.receive(StageActorEnvelope::new(other, AnyMessage::new(StreamRefAck))).expect_err("invalid sender");

  assert!(matches!(error, StreamError::InvalidPartnerActor { .. }));
  assert!(!handoff.is_subscribed());
}

#[test]
fn actor_backed_source_ref_receive_reports_payload_deserialization_error() {
  let system = build_system();
  let (target, _system_messages, _user_messages) = temp_recording_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = ActorBackedSourceRefReceive::new(handoff, system, &target);
  let payload = SerializedMessage::new(SerializerId::from_raw(9_999), None, vec![1, 2, 3]);
  let error = receive
    .receive(StageActorEnvelope::new(target, AnyMessage::new(StreamRefSequencedOnNext::new(0, payload))))
    .expect_err("unknown nested serializer should fail");

  assert!(matches!(error, StreamError::FailedWithContext { .. }));
}

#[test]
fn actor_backed_source_ref_receive_handles_deathwatch_and_unknown_messages() {
  let system = build_system();
  let (target, _system_messages, _user_messages) = temp_recording_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = ActorBackedSourceRefReceive::new(handoff, system, &target);

  let deathwatch_error = receive
    .receive(StageActorEnvelope::new(
      target.clone(),
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(99, 0))),
    ))
    .expect_err("deathwatch should fail active handoff");
  let unknown_error =
    receive.receive(StageActorEnvelope::new(target.clone(), AnyMessage::new(7_u32))).expect_err("unknown message");

  assert!(matches!(deathwatch_error, StreamError::RemoteStreamRefActorTerminated { .. }));
  assert_eq!(unknown_error, StreamError::Failed);

  let terminal = StreamRefHandoff::<u32>::new();
  terminal.close_for_cancel();
  let mut terminal_receive = ActorBackedSourceRefReceive::new(terminal, build_system(), &target);
  assert_eq!(
    terminal_receive.receive(StageActorEnvelope::new(
      target,
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(100, 0))),
    )),
    Ok(())
  );
}
