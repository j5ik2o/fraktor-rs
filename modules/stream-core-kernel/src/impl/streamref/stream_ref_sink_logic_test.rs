use alloc::{borrow::Cow, boxed::Box, vec::Vec};
use core::num::NonZeroU64;

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

use super::{StreamRefEndpointReceive, StreamRefProtocol, StreamRefSinkLogic, StreamRefTargetNotInitializedReceive};
use crate::{
  DemandTracker, DynValue, SinkLogic, StreamError,
  r#impl::streamref::{StreamRefEndpointSlot, StreamRefHandoff},
  materialization::{Completion, StreamDone, StreamFuture},
  stage::{StageActorEnvelope, StageActorReceive},
  stream_ref::{StreamRefCumulativeDemand, StreamRefOnSubscribeHandshake, StreamRefSettings},
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
fn awaiting_remote_subscription_requests_demand_after_subscribe() {
  let handoff = StreamRefHandoff::<u32>::new();
  let mut logic = StreamRefSinkLogic::awaiting_remote_subscription(handoff.clone());
  let mut demand = DemandTracker::new();

  logic.on_start(&mut demand).expect("start");
  assert!(!demand.has_demand());

  handoff.subscribe();
  assert!(logic.on_tick(&mut demand).expect("tick"));
  assert!(demand.has_demand());
}

#[test]
fn awaiting_remote_subscription_rejects_push_before_subscribe() {
  let handoff = StreamRefHandoff::<u32>::new();
  let mut logic = StreamRefSinkLogic::awaiting_remote_subscription(handoff.clone());
  let mut demand = DemandTracker::new();

  let input: DynValue = Box::new(10_u32);
  let error = logic.on_push(input, &mut demand).expect_err("push before subscribe");

  assert_eq!(error, StreamError::WouldBlock);
  assert!(!demand.has_demand());
  handoff.subscribe();
  assert_eq!(handoff.poll_or_drain(), Err(StreamError::WouldBlock));
}

#[test]
fn awaiting_remote_subscription_fails_after_configured_ticks() {
  let handoff = StreamRefHandoff::<u32>::new();
  let mut logic = StreamRefSinkLogic::awaiting_remote_subscription(handoff);
  logic.attach_stream_ref_settings(StreamRefSettings::new().with_subscription_timeout_ticks(1));
  let mut demand = DemandTracker::new();

  let error = logic.on_tick(&mut demand).expect_err("subscription timeout");

  assert!(matches!(error, StreamError::StreamRefSubscriptionTimeout { .. }));
}

#[test]
fn subscribed_sink_completes_materialized_completion() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.subscribe();
  let completion = StreamFuture::<StreamDone>::new();
  let mut logic = StreamRefSinkLogic::subscribed(handoff, Some(completion.clone()));

  logic.on_complete().expect("complete");

  assert!(matches!(completion.value(), Completion::Ready(Ok(_))));
}

#[test]
fn subscribed_sink_respects_configured_buffer_capacity() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.subscribe();
  let mut logic = StreamRefSinkLogic::subscribed(handoff, None);
  logic.attach_stream_ref_settings(StreamRefSettings::new().with_buffer_capacity(1));
  let mut demand = DemandTracker::new();

  let first: DynValue = Box::new(10_u32);
  logic.on_push(first, &mut demand).expect("first element fits capacity");
  let second: DynValue = Box::new(20_u32);
  let error = logic.on_push(second, &mut demand).expect_err("second element exceeds capacity");

  assert_eq!(error, StreamError::BufferOverflow);
}

#[test]
fn awaiting_remote_subscription_with_endpoint_installs_endpoint_actor_once() {
  let system = build_system();
  let endpoint = StreamRefEndpointSlot::new();
  let mut logic =
    StreamRefSinkLogic::awaiting_remote_subscription_with_endpoint(StreamRefHandoff::<u32>::new(), endpoint.clone());

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
fn endpoint_receive_handshake_pairs_partner_and_flushes_completion_on_demand() {
  let system = build_system();
  let (partner, user_messages) = temp_recording_actor(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = StreamRefEndpointReceive::new(handoff.clone(), system, ActorRef::null());
  let demand = NonZeroU64::new(1).expect("demand");

  receive
    .receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(StreamRefOnSubscribeHandshake::new(partner_key))))
    .expect("handshake");
  handoff.complete();
  receive
    .receive(StageActorEnvelope::new(partner, AnyMessage::new(StreamRefCumulativeDemand::new(0, demand))))
    .expect("demand flushes completion");

  assert!(handoff.is_subscribed());
  assert_eq!(*user_messages.lock(), 2);
}

#[test]
fn endpoint_receive_flushes_failure_and_rejects_control_protocols_on_demand() {
  let system = build_system();
  let (partner, user_messages) = temp_recording_actor(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = StreamRefEndpointReceive::new(handoff.clone(), system, ActorRef::null());
  let demand = NonZeroU64::new(1).expect("demand");

  let helper_error = StreamRefEndpointReceive::<u32>::stream_error_from_context("boom");
  assert!(matches!(helper_error, StreamError::FailedWithContext { .. }));
  receive
    .receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(StreamRefOnSubscribeHandshake::new(partner_key))))
    .expect("handshake");
  handoff.push_protocol_for_test(StreamRefProtocol::RemoteStreamFailure { message: Cow::Borrowed("boom") });
  receive
    .receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(StreamRefCumulativeDemand::new(0, demand))))
    .expect("demand flushes failure");

  assert_eq!(*user_messages.lock(), 2);

  let invalid = StreamRefHandoff::<u32>::new();
  let mut invalid_receive = StreamRefEndpointReceive::new(invalid.clone(), build_system(), ActorRef::null());
  let invalid_partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  invalid_receive
    .receive(StageActorEnvelope::new(
      partner.clone(),
      AnyMessage::new(StreamRefOnSubscribeHandshake::new(invalid_partner_key)),
    ))
    .expect("invalid handshake");
  invalid.push_protocol_for_test(StreamRefProtocol::Ack);
  assert_eq!(
    invalid_receive
      .receive(StageActorEnvelope::new(partner, AnyMessage::new(StreamRefCumulativeDemand::new(0, demand)),)),
    Err(StreamError::Failed)
  );
}

#[test]
fn endpoint_receive_rejects_control_protocol_messages_from_ready_flush() {
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = StreamRefEndpointReceive::new(handoff, build_system(), ActorRef::null());

  assert_eq!(receive.flush_protocol_messages(Vec::from([StreamRefProtocol::Ack])), Err(StreamError::Failed));
  assert_eq!(
    receive.flush_protocol_messages(Vec::from([StreamRefProtocol::OnSubscribeHandshake])),
    Err(StreamError::Failed)
  );
}

#[test]
fn endpoint_receive_rejects_unpaired_send_unknown_message_and_active_deathwatch() {
  let system = build_system();
  let (partner, _user_messages) = temp_recording_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  let mut receive = StreamRefEndpointReceive::new(handoff, system, ActorRef::null());
  let demand = NonZeroU64::new(1).expect("demand");

  assert_eq!(
    receive.send_to_partner(StreamRefOnSubscribeHandshake::new(String::from("target"))),
    Err(StreamError::StreamRefTargetNotInitialized)
  );
  assert!(matches!(
    receive
      .receive(StageActorEnvelope::new(partner.clone(), AnyMessage::new(StreamRefCumulativeDemand::new(0, demand)),)),
    Err(StreamError::StreamRefTargetNotInitialized)
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
