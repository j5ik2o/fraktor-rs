use alloc::{borrow::Cow, boxed::Box, vec::Vec};
use core::{fmt::Debug, num::NonZeroU64};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system_with;
use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    messaging::{AnyMessage, system_message::SystemMessage},
    scheduler::SchedulerConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};
use tracing::{
  Event, Id, Metadata, Subscriber,
  field::{Field, Visit},
  level_filters::LevelFilter,
  span::{Attributes, Record},
  subscriber,
};

use super::{StreamRefHandoff, StreamRefProtocol};
use crate::{
  StreamError,
  stage::{CancellationCause, StageActor, StageActorEnvelope, StageActorReceive},
  stream_ref::StreamRefCumulativeDemand,
};

impl<T> StreamRefHandoff<T> {
  pub(crate) fn push_protocol_for_test(&self, protocol: StreamRefProtocol) {
    self.inner.lock().values.push_back(protocol);
  }

  pub(crate) fn pair_endpoint_for_test(&self, got_ref: &'static str) {
    self.inner.lock().endpoint.pair_partner(got_ref).expect("pair endpoint");
  }
}

struct NoopReceive;

impl StageActorReceive for NoopReceive {
  fn receive(&mut self, _envelope: StageActorEnvelope) -> Result<(), StreamError> {
    Ok(())
  }
}

struct EnabledTracingSubscriber;

struct IgnoreFields;

impl Visit for IgnoreFields {
  fn record_debug(&mut self, _field: &Field, _value: &dyn Debug) {}
}

impl Subscriber for EnabledTracingSubscriber {
  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn max_level_hint(&self) -> Option<LevelFilter> {
    Some(LevelFilter::TRACE)
  }

  fn new_span(&self, _span: &Attributes<'_>) -> Id {
    Id::from_u64(1)
  }

  fn record(&self, _span: &Id, _values: &Record<'_>) {}

  fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

  fn event(&self, event: &Event<'_>) {
    let mut fields = IgnoreFields;
    event.record(&mut fields);
  }

  fn enter(&self, _span: &Id) {}

  fn exit(&self, _span: &Id) {}
}

fn with_enabled_tracing<T>(f: impl FnOnce() -> T) -> T {
  subscriber::with_default(EnabledTracingSubscriber, f)
}

struct RecordingSender {
  system_messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
  demand_messages: ArcShared<SpinSyncMutex<Vec<(u64, u64)>>>,
}

impl RecordingSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<Vec<(u64, u64)>>>, Self) {
    let system_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
    let demand_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
    let sender = Self { system_messages: system_messages.clone(), demand_messages: demand_messages.clone() };
    (system_messages, demand_messages, sender)
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(system_message) = message.downcast_ref::<SystemMessage>() {
      self.system_messages.lock().push(system_message.clone());
    }
    if let Some(demand) = message.downcast_ref::<StreamRefCumulativeDemand>() {
      self.demand_messages.lock().push((demand.seq_nr(), demand.demand().get()));
    }
    Ok(SendOutcome::Delivered)
  }
}

struct FailingSender;

impl ActorRefSender for FailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::full(message))
  }
}

struct UnwatchFailingSender {
  system_messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
}

impl ActorRefSender for UnwatchFailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let is_unwatch = matches!(message.downcast_ref::<SystemMessage>(), Some(SystemMessage::Unwatch(_)));
    if let Some(system_message) = message.downcast_ref::<SystemMessage>() {
      self.system_messages.lock().push(system_message.clone());
    }
    if is_unwatch {
      return Err(SendError::full(message));
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
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<Vec<(u64, u64)>>>) {
  let (system_messages, demand_messages, sender) = RecordingSender::new();
  let system_state = system.state();
  let actor_ref =
    ActorRef::from_shared(system.allocate_pid(), ActorRefSenderShared::new(Box::new(sender)), &system_state);
  let _name = system_state.register_temp_actor(actor_ref.clone());
  (actor_ref, system_messages, demand_messages)
}

fn temp_failing_actor(system: &ActorSystem) -> ActorRef {
  let system_state = system.state();
  let actor_ref =
    ActorRef::from_shared(system.allocate_pid(), ActorRefSenderShared::new(Box::new(FailingSender)), &system_state);
  let _name = system_state.register_temp_actor(actor_ref.clone());
  actor_ref
}

fn temp_unwatch_failing_actor(system: &ActorSystem) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>) {
  let system_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let sender = UnwatchFailingSender { system_messages: system_messages.clone() };
  let system_state = system.state();
  let actor_ref =
    ActorRef::from_shared(system.allocate_pid(), ActorRefSenderShared::new(Box::new(sender)), &system_state);
  let _name = system_state.register_temp_actor(actor_ref.clone());
  (actor_ref, system_messages)
}

fn attached_handoff(system: &ActorSystem) -> (StreamRefHandoff<u32>, StageActor) {
  let handoff = StreamRefHandoff::<u32>::new();
  let endpoint_actor = StageActor::new(system, Box::new(NoopReceive));
  handoff.attach_endpoint_actor(endpoint_actor.clone(), None);
  (handoff, endpoint_actor)
}

#[test]
fn poll_or_drain_returns_values_then_completion() {
  let handoff = StreamRefHandoff::new();

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.offer(20_u32), Ok(1));
  assert_eq!(handoff.complete(), 2);

  handoff.record_cumulative_demand();
  assert_eq!(handoff.poll_or_drain(), Ok(Some(10_u32)));
  handoff.record_cumulative_demand();
  assert_eq!(handoff.poll_or_drain(), Ok(Some(20_u32)));
  assert_eq!(handoff.poll_or_drain(), Ok(None));
}

#[test]
fn poll_or_drain_waits_for_cumulative_demand_before_delivering_value() {
  let handoff = StreamRefHandoff::new();

  assert_eq!(handoff.offer(10_u32), Ok(0));

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::WouldBlock));
  handoff.record_cumulative_demand();
  assert_eq!(handoff.poll_or_drain(), Ok(Some(10_u32)));
}

#[test]
fn completion_waits_behind_pending_elements_until_demand_arrives() {
  let handoff = StreamRefHandoff::new();

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.complete(), 1);

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::WouldBlock));
  handoff.record_cumulative_demand();
  assert_eq!(handoff.poll_or_drain(), Ok(Some(10_u32)));
  assert_eq!(handoff.poll_or_drain(), Ok(None));
}

#[test]
fn poll_or_drain_propagates_failure() {
  let handoff = StreamRefHandoff::<u32>::new();

  handoff.fail(StreamError::Failed);

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::Failed));
}

#[test]
fn repeated_failure_and_cancel_keep_first_terminal_failure() {
  let handoff = StreamRefHandoff::<u32>::new();

  handoff.fail(StreamError::Failed);
  assert_eq!(handoff.fail_and_report(StreamError::StreamDetached), StreamError::Failed);
  handoff.close_for_cancel();

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::Failed));
}

#[test]
fn close_for_cancel_is_observed_as_cancellation_not_completion() {
  let handoff = StreamRefHandoff::<u32>::new();

  handoff.close_for_cancel();

  assert_eq!(
    handoff.poll_or_drain(),
    Err(StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() })
  );
}

#[test]
fn close_for_cancel_rejects_additional_publication() {
  let handoff = StreamRefHandoff::<u32>::new();

  handoff.close_for_cancel();

  assert_eq!(
    handoff.offer(10_u32),
    Err(StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() })
  );
}

#[test]
fn offer_rejects_values_beyond_configured_buffer_capacity() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.configure_buffer_capacity(1);

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.offer(20_u32), Err(StreamError::BufferOverflow));
}

#[test]
fn offer_rejects_values_after_normal_completion() {
  let handoff = StreamRefHandoff::<u32>::new();

  assert_eq!(handoff.complete(), 0);
  assert_eq!(handoff.offer(10_u32), Err(StreamError::StreamDetached));
}

#[test]
fn stale_cumulative_demand_after_delivered_sequence_is_ignored() {
  let handoff = StreamRefHandoff::new();
  let demand = NonZeroU64::new(1).expect("demand");

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.record_cumulative_demand_from(0, demand), Ok(()));
  assert_eq!(handoff.drain_ready_protocols().expect("first drain").len(), 1);
  assert_eq!(handoff.record_cumulative_demand_from(0, demand), Ok(()));
  assert_eq!(handoff.offer(20_u32), Ok(1));
  assert!(handoff.drain_ready_protocols().expect("stale demand drain").is_empty());
  assert_eq!(handoff.record_cumulative_demand_from(1, demand), Ok(()));
  assert_eq!(handoff.drain_ready_protocols().expect("second drain").len(), 1);
}

#[test]
fn cumulative_demand_at_current_sequence_accumulates_pending_capacity() {
  let handoff = StreamRefHandoff::new();
  let demand = NonZeroU64::new(1).expect("demand");

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.offer(20_u32), Ok(1));
  assert_eq!(handoff.offer(30_u32), Ok(2));
  assert_eq!(handoff.record_cumulative_demand_from(0, demand), Ok(()));
  assert_eq!(handoff.record_cumulative_demand_from(0, demand), Ok(()));

  assert_eq!(handoff.drain_ready_protocols().expect("drain accumulated demand").len(), 2);
  assert!(handoff.has_pending_protocols());
  assert!(handoff.drain_ready_protocols().expect("demand exhausted").is_empty());
}

#[test]
fn pair_partner_actor_watches_partner_and_sends_demand() {
  let system = build_system();
  let (partner, system_messages, demand_messages) = temp_recording_actor(&system);
  let (handoff, endpoint_actor) = attached_handoff(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  let demand = NonZeroU64::new(3).expect("demand");

  handoff.pair_partner_actor(partner_key, partner).expect("pair partner");
  handoff.send_cumulative_demand_to_partner(5, demand).expect("send demand");

  assert!(handoff.is_subscribed());
  assert_eq!(system_messages.lock().as_slice(), &[SystemMessage::Watch(endpoint_actor.actor_ref().pid())]);
  assert_eq!(demand_messages.lock().as_slice(), &[(5, 3)]);
}

#[test]
fn ensure_partner_actor_accepts_stopped_partner_by_pid() {
  let system = build_system();
  let (partner, _system_messages, _demand_messages) = temp_recording_actor(&system);
  let (other, _other_system_messages, _other_demand_messages) = temp_recording_actor(&system);
  let (handoff, _endpoint_actor) = attached_handoff(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();

  handoff.pair_partner_actor(partner_key, partner.clone()).expect("pair partner");
  system.state().unregister_temp_actor_by_pid(&partner.pid());
  system.state().unregister_temp_actor_by_pid(&other.pid());

  assert_eq!(handoff.ensure_partner_actor(&partner), Ok(()));
  assert!(matches!(handoff.ensure_partner_actor(&other), Err(StreamError::InvalidPartnerActor { .. })));
}

#[test]
fn pair_partner_actor_does_not_pair_when_watch_fails() {
  let system = build_system();
  let partner = temp_failing_actor(&system);
  let (handoff, _endpoint_actor) = attached_handoff(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();

  let _error = handoff.pair_partner_actor(partner_key.clone(), partner).expect_err("watch should fail");

  assert!(!handoff.is_subscribed());
  assert_eq!(handoff.ensure_partner(partner_key), Err(StreamError::StreamRefTargetNotInitialized));
}

#[test]
fn pair_partner_actor_unwatches_partner_when_pairing_fails_after_watch() {
  let system = build_system();
  let (partner, system_messages, _demand_messages) = temp_recording_actor(&system);
  let (handoff, endpoint_actor) = attached_handoff(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  let endpoint_pid = endpoint_actor.actor_ref().pid();
  handoff.pair_endpoint_for_test("already-paired");

  let error = handoff.pair_partner_actor(partner_key, partner).expect_err("pairing should fail");

  assert!(matches!(error, StreamError::InvalidPartnerActor { .. }));
  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid), SystemMessage::Unwatch(endpoint_pid)]);
  assert!(matches!(handoff.offer(10_u32), Err(StreamError::InvalidPartnerActor { .. })));
}

#[test]
fn pair_partner_actor_reports_unwatch_rollback_failure() {
  let system = build_system();
  let (partner, system_messages) = temp_unwatch_failing_actor(&system);
  let (handoff, endpoint_actor) = attached_handoff(&system);
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();
  let endpoint_pid = endpoint_actor.actor_ref().pid();
  handoff.pair_endpoint_for_test("already-paired");

  let error = handoff.pair_partner_actor(partner_key, partner).expect_err("pairing should fail");

  assert!(matches!(error, StreamError::MaterializedResourceRollbackFailed { .. }));
  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid), SystemMessage::Unwatch(endpoint_pid)]);
  assert!(matches!(handoff.offer(10_u32), Err(StreamError::MaterializedResourceRollbackFailed { .. })));
}

#[test]
fn pair_partner_actor_records_duplicate_partner_failure() {
  let system = build_system();
  let (first_partner, _first_system_messages, _first_demand_messages) = temp_recording_actor(&system);
  let (second_partner, _second_system_messages, _second_demand_messages) = temp_recording_actor(&system);
  let (handoff, _endpoint_actor) = attached_handoff(&system);
  let first_partner_key = first_partner.canonical_path().expect("canonical path").to_canonical_uri();
  let second_partner_key = second_partner.canonical_path().expect("canonical path").to_canonical_uri();

  handoff.pair_partner_actor(first_partner_key, first_partner).expect("first partner");

  let error =
    handoff.pair_partner_actor(second_partner_key, second_partner).expect_err("duplicate partner should fail");

  assert!(matches!(error, StreamError::InvalidPartnerActor { .. }));
  assert!(matches!(handoff.offer(10_u32), Err(StreamError::InvalidPartnerActor { .. })));
}

#[test]
fn send_cumulative_demand_without_cleanup_or_partner_reports_partner_unavailable() {
  let system = build_system();
  let handoff = StreamRefHandoff::<u32>::new();
  let demand = NonZeroU64::new(1).expect("demand");

  with_enabled_tracing(|| {
    assert_eq!(
      handoff.send_cumulative_demand_to_partner(0, demand),
      Err(StreamError::StreamRefPartnerUnavailable { seq_nr: 0, demand, reason: "endpoint cleanup missing" })
    );
  });

  let (attached, endpoint_actor) = attached_handoff(&system);
  with_enabled_tracing(|| {
    assert_eq!(
      attached.send_cumulative_demand_to_partner(0, demand),
      Err(StreamError::StreamRefPartnerUnavailable { seq_nr: 0, demand, reason: "partner actor missing" })
    );
  });
  let mut endpoint_ref = endpoint_actor.actor_ref().clone();
  endpoint_ref.try_tell(AnyMessage::new(())).expect("enqueue noop");
  assert_eq!(endpoint_actor.drain_pending(), Ok(()));
}

#[test]
fn send_cumulative_demand_reports_partner_send_failure() {
  let system = build_system();
  let partner = temp_failing_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  let endpoint_actor = StageActor::new(&system, Box::new(NoopReceive));
  let demand = NonZeroU64::new(1).expect("demand");
  handoff.attach_endpoint_actor(endpoint_actor, Some(partner));

  assert_eq!(handoff.send_cumulative_demand_to_partner(0, demand), Err(StreamError::WouldBlock));
}

#[test]
fn subscribe_after_remote_pair_records_invalid_partner_failure() {
  let system = build_system();
  let (partner, _system_messages, _demand_messages) = temp_recording_actor(&system);
  let handoff = StreamRefHandoff::<u32>::new();
  let partner_key = partner.canonical_path().expect("canonical path").to_canonical_uri();

  handoff.pair_partner_actor(partner_key, partner).expect("pair partner");
  handoff.subscribe();

  assert!(matches!(handoff.offer(10_u32), Err(StreamError::InvalidPartnerActor { .. })));
}

#[test]
fn cleanup_after_terminal_delivery_without_shutdown_is_noop() {
  let handoff = StreamRefHandoff::<u32>::new();

  assert_eq!(handoff.cleanup_after_terminal_delivery(), Ok(()));
}

#[test]
fn terminal_cleanup_failures_are_reported_from_completion_cancel_and_failure_paths() {
  let system = build_system();
  let watcher = temp_failing_actor(&system);
  let (complete_handoff, complete_endpoint) = attached_handoff(&system);
  system
    .state()
    .send_system_message(complete_endpoint.actor_ref().pid(), SystemMessage::Watch(watcher.pid()))
    .expect("register watcher");

  complete_handoff.complete();
  let error = complete_handoff.cleanup_after_terminal_delivery().expect_err("terminal cleanup failure");
  assert!(matches!(error, StreamError::MaterializedResourceRollbackFailed { .. }));
  assert_eq!(error.materialization_cleanup_failure(), Some(&StreamError::WouldBlock));
  assert!(matches!(complete_handoff.poll_or_drain(), Err(StreamError::MaterializedResourceRollbackFailed { .. })));

  let watcher = temp_failing_actor(&system);
  let (cancel_handoff, cancel_endpoint) = attached_handoff(&system);
  system
    .state()
    .send_system_message(cancel_endpoint.actor_ref().pid(), SystemMessage::Watch(watcher.pid()))
    .expect("register watcher");

  cancel_handoff.close_for_cancel();
  assert!(matches!(cancel_handoff.poll_or_drain(), Err(StreamError::MaterializedResourceRollbackFailed { .. })));

  let watcher = temp_failing_actor(&system);
  let (failed_handoff, failed_endpoint) = attached_handoff(&system);
  system
    .state()
    .send_system_message(failed_endpoint.actor_ref().pid(), SystemMessage::Watch(watcher.pid()))
    .expect("register watcher");

  let error = failed_handoff.fail_and_report(StreamError::Failed);
  assert!(matches!(error, StreamError::MaterializedResourceRollbackFailed { .. }));
}

#[test]
fn poll_or_drain_reports_completion_cleanup_failure() {
  let system = build_system();
  let watcher = temp_failing_actor(&system);
  let (handoff, endpoint) = attached_handoff(&system);
  system
    .state()
    .send_system_message(endpoint.actor_ref().pid(), SystemMessage::Watch(watcher.pid()))
    .expect("register watcher");

  handoff.complete();
  let error = handoff.poll_or_drain().expect_err("completion cleanup failure");
  assert!(matches!(error, StreamError::MaterializedResourceRollbackFailed { .. }));
  assert_eq!(error.materialization_cleanup_failure(), Some(&StreamError::WouldBlock));
}

#[test]
fn remote_enqueue_paths_reject_failure_closed_and_overflow_states() {
  let failed = StreamRefHandoff::<u32>::new();
  failed.fail(StreamError::Failed);
  assert_eq!(failed.enqueue_remote_element(0, 10), Err(StreamError::Failed));
  assert_eq!(failed.enqueue_remote_completed(0), Err(StreamError::Failed));
  assert!(matches!(failed.drain_ready_protocols(), Err(StreamError::Failed)));

  let closed = StreamRefHandoff::<u32>::new();
  closed.complete();
  assert_eq!(closed.enqueue_remote_element(0, 10), Err(StreamError::StreamDetached));

  let full = StreamRefHandoff::<u32>::new();
  full.configure_buffer_capacity(1);
  assert_eq!(full.enqueue_remote_element(0, 10), Ok(()));
  assert_eq!(full.enqueue_remote_element(1, 20), Err(StreamError::BufferOverflow));
}

#[test]
fn poll_or_drain_rejects_control_protocols_and_remote_failure() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.push_protocol_for_test(StreamRefProtocol::Ack);

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::Failed));

  let failed = StreamRefHandoff::<u32>::new();
  failed.push_protocol_for_test(StreamRefProtocol::RemoteStreamFailure { message: Cow::Borrowed("boom") });
  let error = failed.poll_or_drain().expect_err("remote failure");
  assert!(matches!(error, StreamError::FailedWithContext { .. }));
  assert!(error.to_string().contains("boom"));
}

#[test]
fn drain_ready_protocols_drains_failure_and_rejects_control_protocol() {
  let failed = StreamRefHandoff::<u32>::new();
  failed.push_protocol_for_test(StreamRefProtocol::RemoteStreamFailure { message: Cow::Borrowed("boom") });
  let messages = failed.drain_ready_protocols().expect("drain failure protocol");

  assert_eq!(messages.len(), 1);
  assert!(matches!(messages.first(), Some(StreamRefProtocol::RemoteStreamFailure { .. })));

  let invalid = StreamRefHandoff::<u32>::new();
  invalid.push_protocol_for_test(StreamRefProtocol::OnSubscribeHandshake);
  assert!(matches!(invalid.drain_ready_protocols(), Err(StreamError::Failed)));
}
