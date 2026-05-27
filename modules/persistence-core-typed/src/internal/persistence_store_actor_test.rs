use alloc::{
  collections::BTreeSet,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::actor::{
  Actor, ActorCell, ActorContext, Pid,
  actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
  error::{ActorError, SendError},
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
};
use fraktor_actor_core_typed_rs::TypedActorRef;
use fraktor_persistence_core_kernel_rs::{
  error::PersistenceError,
  journal::{JournalError, JournalMessage, JournalResponse},
  persistent::{Eventsourced, PersistentActor, PersistentRepr},
  snapshot::{SnapshotError, SnapshotMetadata, SnapshotResponse, SnapshotSelectionCriteria},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedLock, SpinSyncMutex};

use super::PersistenceStoreActor;
use crate::{
  BackoffConfig, EventSourcedSignal, PersistenceEffectorConfig, PersistenceId, internal::PersistenceStoreReply,
};

type ReplyMessages = ArcShared<SpinSyncMutex<Vec<AnyMessage>>>;
type TestReply = PersistenceStoreReply<u32, u32>;

struct RecordingSender {
  messages: ReplyMessages,
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

fn reply_ref() -> (TypedActorRef<TestReply>, ReplyMessages) {
  let (actor_ref, messages) = actor_ref(Pid::new(200, 1));
  (TypedActorRef::from_untyped(actor_ref), messages)
}

fn actor_ref(pid: Pid) -> (ActorRef, ReplyMessages) {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let sender = ActorRefSenderShared::from_shared_lock(SharedLock::new_with_driver::<
    SpinSyncMutex<Box<dyn ActorRefSender>>,
  >(Box::new(RecordingSender { messages: messages.clone() })));
  (ActorRef::new(pid, sender), messages)
}

fn store_actor(config: PersistenceEffectorConfig<u32, u32, ()>) -> PersistenceStoreActor<u32, u32, ()> {
  let (reply_to, _messages) = reply_ref();
  PersistenceStoreActor::new(config, reply_to)
}

fn config() -> PersistenceEffectorConfig<u32, u32, ()> {
  PersistenceEffectorConfig::new(PersistenceId::of_unique_id("typed-store-test"), 0_u32, |state, event| state + event)
}

fn tagged_config() -> PersistenceEffectorConfig<u32, u32, ()> {
  config().with_tagger(|event| BTreeSet::from([format!("event-{event}")]))
}

fn repr(sequence_nr: u64) -> PersistentRepr {
  PersistentRepr::new("typed-store-test", sequence_nr, ArcShared::new(7_u32))
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_context() -> ActorContext<'static> {
  let system = create_noop_actor_system();
  let state = system.state();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(state.clone(), pid, None, "typed-store-test".into(), &props)
    .expect("actor cell should be created");
  state.register_cell(cell);
  ActorContext::new(&system, pid)
}

fn bind_store_refs(
  actor: &mut PersistenceStoreActor<u32, u32, ()>,
  ctx: &mut ActorContext<'_>,
  journal_ref: ActorRef,
  snapshot_ref: ActorRef,
) {
  actor.context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind persistence refs");
  actor.start_recovery(ctx).expect("start recovery");
  actor
    .handle_snapshot_response(&SnapshotResponse::LoadSnapshotResult { snapshot: None, to_sequence_nr: u64::MAX }, ctx);
  actor.handle_journal_response(&JournalResponse::RecoverySuccess { highest_sequence_nr: 0 });
}

fn capture_write_request(messages: &ReplyMessages) -> (PersistentRepr, u32) {
  let messages = messages.lock();
  let message = messages.last().expect("journal write request");
  let request = message.payload().downcast_ref::<JournalMessage>().expect("journal message");
  let JournalMessage::WriteMessages { messages, instance_id, .. } = request else {
    panic!("expected write messages request");
  };
  let repr = messages[0].payload()[0].clone();
  (repr, *instance_id)
}

fn capture_delete_request(messages: &ReplyMessages) -> u64 {
  let messages = messages.lock();
  let message = messages.last().expect("journal delete request");
  let request = message.payload().downcast_ref::<JournalMessage>().expect("journal message");
  let JournalMessage::DeleteMessagesTo { to_sequence_nr, .. } = request else {
    panic!("expected delete messages request");
  };
  *to_sequence_nr
}

#[test]
fn persist_failure_callback_emits_event_sourced_signal_and_defaults_to_fatal() {
  let mut actor = store_actor(config());
  let (reply_to, messages) = reply_ref();
  actor.pending_persist_reply = Some(reply_to);
  let cause = JournalError::WriteFailed("write failed".to_string());
  let repr = repr(2);

  actor.on_persist_failure(&cause, &repr);
  let error = actor.persist_failure_error(&cause, &repr);

  assert!(matches!(error, ActorError::Fatal(message)
    if message.as_str().contains("persistent store stopped after write failure")
      && message.as_str().contains("sequence number 2")));
  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("store reply");
  assert!(matches!(reply, PersistenceStoreReply::EventSourced {
    signal: EventSourcedSignal::JournalPersistFailed { error }
  } if *error == PersistenceError::from(cause)));
}

#[test]
fn persist_failure_error_is_recoverable_when_persist_failure_backoff_is_configured() {
  let actor = store_actor(config().on_persist_failure(BackoffConfig::default()));
  let cause = JournalError::WriteFailed("write failed".to_string());
  let repr = repr(3);

  let error = actor.persist_failure_error(&cause, &repr);

  assert!(matches!(error, ActorError::Recoverable(message)
    if message.as_str().contains("persistent store restarted after write failure")
      && message.as_str().contains("sequence number 3")));
}

#[test]
fn persist_rejection_callback_emits_rejected_signal_with_identity_and_cause() {
  let mut actor = store_actor(config());
  let (reply_to, messages) = reply_ref();
  actor.pending_persist_reply = Some(reply_to);
  let cause = JournalError::WriteFailed("write rejected".to_string());
  let repr = repr(4);

  actor.on_persist_rejected(&cause, &repr);

  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("store reply");
  assert!(matches!(reply, PersistenceStoreReply::EventSourced {
    signal: EventSourcedSignal::JournalPersistRejected { .. },
  }));
  let PersistenceStoreReply::EventSourced { signal: EventSourcedSignal::JournalPersistRejected { error } } = reply
  else {
    panic!("unexpected store reply");
  };
  assert_eq!(error.persistence_id().as_str(), "typed-store-test");
  assert_eq!(error.sequence_nr(), 4);
  assert_eq!(error.cause(), &PersistenceError::from(cause));
}

#[test]
fn recovery_completion_emits_event_sourced_signal_before_ready_reply() {
  let (reply_to, messages) = reply_ref();
  let mut actor = PersistenceStoreActor::new(config(), reply_to);

  actor.on_recovery_completed();

  let messages = messages.lock();
  assert_eq!(messages.len(), 2);
  let first = messages[0].payload().downcast_ref::<TestReply>().expect("first reply");
  assert!(matches!(first, PersistenceStoreReply::EventSourced { signal: EventSourcedSignal::RecoveryCompleted }));
  let second = messages[1].payload().downcast_ref::<TestReply>().expect("second reply");
  assert!(matches!(second, PersistenceStoreReply::RecoveryCompleted { state: 0, sequence_nr: 0 }));
}

#[test]
fn snapshot_success_and_delete_success_emit_public_event_sourced_signals() {
  let mut actor = store_actor(config());
  let (snapshot_reply_to, snapshot_messages) = reply_ref();
  let snapshot_metadata = SnapshotMetadata::new("typed-store-test", 5, 0);
  actor.pending_snapshot = Some((99, snapshot_reply_to));

  actor.on_snapshot_saved(&snapshot_metadata);

  let messages = snapshot_messages.lock();
  assert_eq!(messages.len(), 2);
  let first = messages[0].payload().downcast_ref::<TestReply>().expect("snapshot signal");
  assert!(matches!(first, PersistenceStoreReply::EventSourced {
      signal: EventSourcedSignal::SnapshotCompleted { metadata }
    } if metadata == &snapshot_metadata));
  let second = messages[1].payload().downcast_ref::<TestReply>().expect("snapshot reply");
  assert!(matches!(second, PersistenceStoreReply::PersistedSnapshot { snapshot: 99, .. }));
  drop(messages);

  let (delete_reply_to, delete_messages) = reply_ref();
  let criteria = SnapshotSelectionCriteria::new(7, u64::MAX, 0, 0);
  actor.pending_delete_snapshots = Some((7, delete_reply_to));

  actor.on_snapshots_deleted(&criteria);

  let messages = delete_messages.lock();
  assert_eq!(messages.len(), 2);
  let first = messages[0].payload().downcast_ref::<TestReply>().expect("delete signal");
  assert!(matches!(first, PersistenceStoreReply::EventSourced {
      signal: EventSourcedSignal::DeleteSnapshotsCompleted { criteria: actual }
    } if actual == &criteria));
  let second = messages[1].payload().downcast_ref::<TestReply>().expect("delete reply");
  assert!(matches!(second, PersistenceStoreReply::DeletedSnapshots { to_sequence_nr: 7 }));
}

#[test]
fn snapshot_failure_emits_public_event_sourced_failure_signal() {
  let mut actor = store_actor(config());
  let (reply_to, messages) = reply_ref();
  actor.pending_snapshot = Some((11, reply_to));
  let cause = SnapshotError::SaveFailed("disk full".to_string());

  actor.on_snapshot_failure(&cause);

  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("snapshot failure reply");
  assert!(matches!(reply, PersistenceStoreReply::EventSourced {
      signal: EventSourcedSignal::SnapshotFailed { metadata: Some(metadata), error }
    } if metadata.persistence_id() == "typed-store-test" && *error == PersistenceError::from(cause)));
}

#[test]
fn journal_write_failure_response_emits_failure_signal_through_store_response_path() {
  let mut actor = store_actor(config());
  let (journal_ref, journal_messages) = actor_ref(Pid::new(201, 1));
  let (snapshot_ref, _snapshot_messages) = actor_ref(Pid::new(202, 1));
  let mut ctx = build_context();
  bind_store_refs(&mut actor, &mut ctx, journal_ref, snapshot_ref);
  journal_messages.lock().clear();
  let (reply_to, messages) = reply_ref();
  actor.persist_event(&mut ctx, 7, reply_to).expect("persist event should send write request");
  let (repr, instance_id) = capture_write_request(&journal_messages);
  let cause = JournalError::WriteFailed("write failed".to_string());

  actor.handle_journal_response(&JournalResponse::WriteMessageFailure { repr, cause: cause.clone(), instance_id });

  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("failure reply");
  assert!(matches!(reply, PersistenceStoreReply::EventSourced {
    signal: EventSourcedSignal::JournalPersistFailed { error }
  } if *error == PersistenceError::from(cause)));
}

#[test]
fn journal_rejection_response_emits_rejection_signal_through_store_response_path() {
  let mut actor = store_actor(config());
  let (journal_ref, journal_messages) = actor_ref(Pid::new(203, 1));
  let (snapshot_ref, _snapshot_messages) = actor_ref(Pid::new(204, 1));
  let mut ctx = build_context();
  bind_store_refs(&mut actor, &mut ctx, journal_ref, snapshot_ref);
  journal_messages.lock().clear();
  let (reply_to, messages) = reply_ref();
  actor.persist_event(&mut ctx, 8, reply_to).expect("persist event should send write request");
  let (repr, instance_id) = capture_write_request(&journal_messages);
  let cause = JournalError::WriteFailed("write rejected".to_string());

  actor.handle_journal_response(&JournalResponse::WriteMessageRejected { repr, cause: cause.clone(), instance_id });

  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("rejection reply");
  let PersistenceStoreReply::EventSourced { signal: EventSourcedSignal::JournalPersistRejected { error } } = reply
  else {
    panic!("unexpected store reply");
  };
  assert_eq!(error.persistence_id().as_str(), "typed-store-test");
  assert_eq!(error.sequence_nr(), 1);
  assert_eq!(error.cause(), &PersistenceError::from(cause));
}

#[test]
fn persisted_event_reply_includes_tags_selected_by_config_tagger() {
  let mut actor = store_actor(tagged_config());
  let (journal_ref, journal_messages) = actor_ref(Pid::new(205, 1));
  let (snapshot_ref, _snapshot_messages) = actor_ref(Pid::new(206, 1));
  let mut ctx = build_context();
  bind_store_refs(&mut actor, &mut ctx, journal_ref, snapshot_ref);
  journal_messages.lock().clear();
  let (reply_to, messages) = reply_ref();
  actor.persist_event(&mut ctx, 7, reply_to).expect("persist event should send write request");
  let (repr, instance_id) = capture_write_request(&journal_messages);

  actor.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr, instance_id });

  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("persisted reply");
  let PersistenceStoreReply::PersistedEvents { published_events, .. } = reply else {
    panic!("unexpected store reply");
  };
  assert_eq!(published_events[0].tags(), &BTreeSet::from([String::from("event-7")]));
}

#[test]
fn batch_persist_type_mismatch_stops_remaining_state_mutation() {
  let mut actor = store_actor(config());
  let (journal_ref, journal_messages) = actor_ref(Pid::new(207, 1));
  let (snapshot_ref, _snapshot_messages) = actor_ref(Pid::new(208, 1));
  let mut ctx = build_context();
  bind_store_refs(&mut actor, &mut ctx, journal_ref, snapshot_ref);
  journal_messages.lock().clear();
  let (reply_to, messages) = reply_ref();
  actor.persist_events(&mut ctx, Vec::from([7, 8]), reply_to).expect("persist events should send write request");
  let (first_repr, second_repr, instance_id) = {
    let messages = journal_messages.lock();
    let message = messages.last().expect("journal write request");
    let request = message.payload().downcast_ref::<JournalMessage>().expect("journal message");
    let JournalMessage::WriteMessages { messages, instance_id, .. } = request else {
      panic!("expected write messages request");
    };
    (messages[0].payload()[0].clone(), messages[0].payload()[1].clone(), *instance_id)
  };
  let bad_repr =
    PersistentRepr::new(first_repr.persistence_id(), first_repr.sequence_nr(), ArcShared::new(String::from("not-u32")));

  actor.reply_persist_type_mismatch(&bad_repr);
  actor.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: first_repr, instance_id });
  actor.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: second_repr, instance_id });

  assert_eq!(actor.state, 0);
  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("failure reply");
  assert!(matches!(reply, PersistenceStoreReply::EventSourced {
    signal: EventSourcedSignal::JournalPersistFailed { .. },
  }));
}

#[test]
fn journal_delete_messages_response_emits_delete_events_signal_through_store_response_path() {
  let mut actor = store_actor(config());
  let (journal_ref, journal_messages) = actor_ref(Pid::new(209, 1));
  let (snapshot_ref, _snapshot_messages) = actor_ref(Pid::new(210, 1));
  let mut ctx = build_context();
  bind_store_refs(&mut actor, &mut ctx, journal_ref, snapshot_ref);
  journal_messages.lock().clear();
  let (reply_to, messages) = reply_ref();

  actor.delete_events_to(&mut ctx, 9, reply_to).expect("delete events should send journal request");
  let to_sequence_nr = capture_delete_request(&journal_messages);
  actor.handle_journal_response(&JournalResponse::DeleteMessagesSuccess { to_sequence_nr });

  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("delete events reply");
  assert!(matches!(reply, PersistenceStoreReply::EventSourced {
    signal: EventSourcedSignal::DeleteEventsCompleted { to_sequence_nr: 9 },
  }));
}

#[test]
fn journal_delete_messages_failure_emits_delete_events_failed_signal_through_store_response_path() {
  let mut actor = store_actor(config());
  let (journal_ref, journal_messages) = actor_ref(Pid::new(209, 1));
  let (snapshot_ref, _snapshot_messages) = actor_ref(Pid::new(210, 1));
  let mut ctx = build_context();
  bind_store_refs(&mut actor, &mut ctx, journal_ref, snapshot_ref);
  journal_messages.lock().clear();
  let (reply_to, messages) = reply_ref();
  let cause = JournalError::WriteFailed("delete failed".to_string());

  actor.delete_events_to(&mut ctx, 10, reply_to).expect("delete events should send journal request");
  let to_sequence_nr = capture_delete_request(&journal_messages);
  actor.handle_journal_response(&JournalResponse::DeleteMessagesFailure { cause: cause.clone(), to_sequence_nr });

  let messages = messages.lock();
  assert_eq!(messages.len(), 1);
  let reply = messages[0].payload().downcast_ref::<TestReply>().expect("delete events failure reply");
  assert!(matches!(reply, PersistenceStoreReply::EventSourced {
    signal: EventSourcedSignal::DeleteEventsFailed { to_sequence_nr: 10, error }
  } if *error == PersistenceError::from(cause)));
}
