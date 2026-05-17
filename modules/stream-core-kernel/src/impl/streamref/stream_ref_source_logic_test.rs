use alloc::boxed::Box;

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

use super::{StreamRefEndpointReceive, StreamRefSourceLogic, StreamRefTargetNotInitializedReceive};
use crate::{
  SourceLogic, StreamError,
  r#impl::streamref::{StreamRefEndpointSlot, StreamRefHandoff},
  stage::{StageActorEnvelope, StageActorReceive},
  stream_ref::{
    StreamRefOnSubscribeHandshake, StreamRefRemoteStreamCompleted, StreamRefRemoteStreamFailure,
    StreamRefSequencedOnNext, StreamRefSettings,
  },
};

struct RecordingSender {
  user_messages: ArcShared<SpinSyncMutex<usize>>,
}

impl RecordingSender {
  fn new() -> (ArcShared<SpinSyncMutex<usize>>, Self) {
    let user_messages = ArcShared::new(SpinSyncMutex::new(0_usize));
    (user_messages.clone(), Self { user_messages })
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if message.downcast_ref::<SystemMessage>().is_none() {
      *self.user_messages.lock() += 1;
    }
    Ok(SendOutcome::Delivered)
  }
}

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  create_noop_actor_system_with(|config| config.with_scheduler_config(scheduler))
}

fn temp_recording_actor(system: &ActorSystem) -> (ActorRef, ArcShared<SpinSyncMutex<usize>>) {
  let (user_messages, sender) = RecordingSender::new();
  let system_state = system.state();
  let actor_ref =
    ActorRef::from_shared(system.allocate_pid(), ActorRefSenderShared::new(Box::new(sender)), &system_state);
  let _name = system_state.register_temp_actor(actor_ref.clone());
  (actor_ref, user_messages)
}

#[test]
fn awaiting_remote_subscription_fails_after_configured_ticks() {
  let handoff = StreamRefHandoff::<u32>::new();
  let mut logic = StreamRefSourceLogic::awaiting_remote_subscription(handoff);
  logic.attach_stream_ref_settings(StreamRefSettings::new().with_subscription_timeout_ticks(1));

  let error = logic.pull().expect_err("subscription timeout");

  assert!(matches!(error, StreamError::StreamRefSubscriptionTimeout { .. }));
}

#[test]
fn subscribed_source_polls_values_until_completion() {
  let handoff = StreamRefHandoff::new();
  handoff.subscribe();
  handoff.offer(42_u32).expect("offer");
  handoff.complete();
  let mut logic = StreamRefSourceLogic::subscribed(handoff);

  assert!(logic.pull().expect("value").is_some());
  assert!(logic.pull().expect("complete").is_none());
}

#[test]
fn subscribed_source_propagates_handoff_failure() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.subscribe();
  handoff.fail(StreamError::Failed);
  let mut logic = StreamRefSourceLogic::subscribed(handoff);

  assert_eq!(logic.pull().expect_err("handoff failure"), StreamError::Failed);
}

#[test]
fn stream_ref_source_does_not_drain_on_shutdown() {
  let handoff = StreamRefHandoff::<u32>::new();
  let logic = StreamRefSourceLogic::subscribed(handoff);

  assert!(!logic.should_drain_on_shutdown());
}

#[test]
fn awaiting_remote_subscription_with_endpoint_installs_endpoint_actor_once() {
  let system = build_system();
  let endpoint = StreamRefEndpointSlot::new();
  let mut logic =
    StreamRefSourceLogic::awaiting_remote_subscription_with_endpoint(StreamRefHandoff::<u32>::new(), endpoint.clone());

  logic.attach_actor_system(system.clone());
  let first = endpoint.actor_ref().expect("endpoint actor");
  logic.attach_actor_system(system);

  assert_eq!(endpoint.actor_ref().expect("endpoint actor remains"), first);
}

#[test]
fn target_not_initialized_receive_reports_uninitialized() {
  let mut receive = StreamRefTargetNotInitializedReceive;

  assert_eq!(
    receive.receive(StageActorEnvelope::new(ActorRef::null(), AnyMessage::new(7_u32))),
    Err(StreamError::StreamRefTargetNotInitialized)
  );
}

#[test]
fn endpoint_receive_handshake_pairs_partner_and_accepts_terminal_messages() {
  let system = build_system();
  let (partner, user_messages) = temp_recording_actor(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = StreamRefEndpointReceive::new(handoff.clone(), system, ActorRef::null());

  receive
    .receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(StreamRefOnSubscribeHandshake::new(partner_key))))
    .expect("handshake");
  receive
    .receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(StreamRefRemoteStreamCompleted::new(0))))
    .expect("completion");

  assert!(handoff.is_subscribed());
  assert_eq!(*user_messages.lock(), 1);
  assert_eq!(handoff.poll_or_drain(), Ok(None));

  let failed = StreamRefHandoff::<u32>::new();
  let mut failed_receive = StreamRefEndpointReceive::new(failed.clone(), build_system(), ActorRef::null());
  let failed_partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  failed_receive
    .receive(StageActorEnvelope::new(
      partner.clone(),
      AnyMessage::new(StreamRefOnSubscribeHandshake::new(failed_partner_key)),
    ))
    .expect("failed handshake");
  failed_receive
    .receive(StageActorEnvelope::new(partner, AnyMessage::new(StreamRefRemoteStreamFailure::new(String::from("boom")))))
    .expect("failure");
  assert!(matches!(failed.poll_or_drain(), Err(StreamError::FailedWithContext { .. })));
}

#[test]
fn endpoint_receive_reports_payload_deserialization_error() {
  let system = build_system();
  let (partner, _user_messages) = temp_recording_actor(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = StreamRefEndpointReceive::new(handoff, system, ActorRef::null());
  let payload = SerializedMessage::new(SerializerId::from_raw(9_999), None, vec![1, 2, 3]);

  let helper_error = StreamRefEndpointReceive::<u32>::stream_error_from_context("boom");
  assert!(matches!(helper_error, StreamError::FailedWithContext { .. }));
  receive
    .receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(StreamRefOnSubscribeHandshake::new(partner_key))))
    .expect("handshake");
  let error = receive
    .receive(StageActorEnvelope::new(partner, AnyMessage::new(StreamRefSequencedOnNext::new(0, payload))))
    .expect_err("unknown nested serializer should fail");

  assert!(matches!(error, StreamError::FailedWithContext { .. }));
}

#[test]
fn endpoint_receive_rejects_unpaired_send_invalid_sender_unknown_message_and_active_deathwatch() {
  let system = build_system();
  let (partner, _user_messages) = temp_recording_actor(&system);
  let (other, _other_messages) = temp_recording_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = StreamRefEndpointReceive::new(handoff, system, ActorRef::null());

  assert_eq!(
    receive.send_to_partner(StreamRefRemoteStreamCompleted::new(0)),
    Err(StreamError::StreamRefTargetNotInitialized)
  );

  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  receive
    .receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(StreamRefOnSubscribeHandshake::new(partner_key))))
    .expect("handshake");

  assert!(matches!(
    receive.receive(StageActorEnvelope::new(other, AnyMessage::new(StreamRefRemoteStreamCompleted::new(0)))),
    Err(StreamError::InvalidPartnerActor { .. })
  ));
  assert_eq!(
    receive.receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(7_u32))),
    Err(StreamError::Failed)
  );
  assert!(matches!(
    receive.receive(StageActorEnvelope::new(
      partner,
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(99, 0))),
    )),
    Err(StreamError::RemoteStreamRefActorTerminated { .. })
  ));
}

#[test]
fn endpoint_receive_ignores_deathwatch_after_terminal_handoff() {
  let system = build_system();
  let (partner, _user_messages) = temp_recording_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.close_for_cancel();
  let mut receive = StreamRefEndpointReceive::new(handoff, system, ActorRef::null());

  assert_eq!(
    receive.receive(StageActorEnvelope::new(
      partner,
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(100, 0))),
    )),
    Ok(())
  );
}
