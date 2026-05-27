use alloc::{collections::BTreeSet, vec::Vec};
use std::{
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{error::ActorError, extension::ExtensionInstallers, scheduler::SchedulerConfig, setup::ActorSystemConfig},
  event::stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
};
use fraktor_actor_core_typed_rs::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_persistence_core_kernel_rs::{
  error::PersistenceError, extension::PersistenceExtensionInstaller, journal::InMemoryJournal,
  snapshot::InMemorySnapshotStore,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::{
  EventRejectedError, EventSourcedSignal, PersistenceEffector, PersistenceEffectorConfig,
  PersistenceEffectorMessageAdapter, PersistenceEffectorSignal, PersistenceId, PublishedEvent, RetentionCriteria,
};

#[test]
fn retention_delete_to_returns_none_for_zero_snapshot_interval() {
  let retention_criteria = RetentionCriteria::snapshot_every(0, 1);

  let actual = PersistenceEffector::<(), (), ()>::retention_delete_to(retention_criteria, 10);

  assert_eq!(actual, None);
}

#[test]
fn retention_delete_to_returns_none_for_zero_keep_snapshots() {
  let retention_criteria = RetentionCriteria::snapshot_every(2, 0);

  let actual = PersistenceEffector::<(), (), ()>::retention_delete_to(retention_criteria, 10);

  assert_eq!(actual, None);
}

#[test]
fn retention_delete_to_returns_none_before_first_snapshot_interval() {
  let retention_criteria = RetentionCriteria::snapshot_every(5, 1);

  let actual = PersistenceEffector::<(), (), ()>::retention_delete_to(retention_criteria, 3);

  assert_eq!(actual, None);
}

#[test]
fn event_publishing_enabled_publishes_persisted_event_to_system_event_stream() {
  let recorded_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let persisted_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = aggregate_props("published-event", persisted_events.clone(), true);
  let system = typed_persistence_system(&props);
  let subscriber = subscriber_handle(RecordingSubscriber::new(recorded_events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);
  let mut guardian = system.user_guardian_ref();
  guardian.try_tell(AggregateCommand::Add(7)).expect("persist command should be accepted");
  wait_until(|| persisted_events.lock().as_slice() == [7]);
  assert!(contains_published_event(&recorded_events, "published-event", 1, 7));

  system.terminate().expect("terminate");
}

#[test]
fn event_publishing_disabled_does_not_publish_persisted_event_to_system_event_stream() {
  let recorded_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let persisted_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = aggregate_props("unpublished-event", persisted_events.clone(), false);
  let system = typed_persistence_system(&props);
  let subscriber = subscriber_handle(RecordingSubscriber::new(recorded_events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);
  let mut guardian = system.user_guardian_ref();
  guardian.try_tell(AggregateCommand::Add(9)).expect("persist command should be accepted");
  wait_until(|| persisted_events.lock().as_slice() == [9]);
  assert!(!contains_published_event(&recorded_events, "unpublished-event", 1, 9));

  system.terminate().expect("terminate");
}

#[test]
fn journal_rejection_is_exposed_as_distinct_event_sourced_signal() {
  let error = EventRejectedError::new(
    PersistenceId::of_unique_id("pid-rejected"),
    3,
    PersistenceError::StateMachine("journal rejected".into()),
  );
  let signal = PersistenceEffectorSignal::<u32, u32>::EventSourced {
    signal: EventSourcedSignal::JournalPersistRejected { error: error.clone() },
  };
  assert!(
    matches!(
      signal,
      PersistenceEffectorSignal::EventSourced {
        signal: EventSourcedSignal::JournalPersistRejected { error: actual }
      } if actual == error
    ),
    "journal rejection must not be collapsed into generic Failed",
  );
}

#[test]
fn journal_persist_failure_is_recoverable_when_backoff_is_enabled() {
  let signal =
    EventSourcedSignal::JournalPersistFailed { error: PersistenceError::StateMachine(String::from("journal failed")) };

  let actual = PersistenceEffector::<(), (), ()>::event_sourced_signal_behavior(&signal, true);

  assert!(matches!(actual, Err(ActorError::Recoverable(_))));
}

#[test]
fn journal_persist_failure_is_fatal_when_backoff_is_disabled() {
  let signal =
    EventSourcedSignal::JournalPersistFailed { error: PersistenceError::StateMachine(String::from("journal failed")) };

  let actual = PersistenceEffector::<(), (), ()>::event_sourced_signal_behavior(&signal, false);

  assert!(matches!(actual, Err(ActorError::Fatal(_))));
}

#[derive(Clone, Debug)]
enum AggregateCommand {
  Add(u32),
  Signal(PersistenceEffectorSignal<u32, u32>),
}

struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  const fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

fn aggregate_props(
  persistence_id: &str,
  persisted_events: ArcShared<SpinSyncMutex<Vec<u32>>>,
  event_publishing: bool,
) -> TypedProps<AggregateCommand> {
  let config = PersistenceEffectorConfig::new(PersistenceId::of_unique_id(persistence_id), 0_u32, apply_event)
    .with_message_adapter(message_adapter())
    .with_event_publishing(event_publishing);

  PersistenceEffector::props(config, move |_state, effector| Ok(aggregate_behavior(effector, persisted_events.clone())))
}

fn aggregate_behavior(
  effector: PersistenceEffector<u32, u32, AggregateCommand>,
  persisted_events: ArcShared<SpinSyncMutex<Vec<u32>>>,
) -> Behavior<AggregateCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | AggregateCommand::Add(value) => {
      let persisted_events = persisted_events.clone();
      effector.persist_event(ctx, *value, move |event| {
        persisted_events.lock().push(*event);
        Ok(Behaviors::same())
      })
    },
    | AggregateCommand::Signal(_) => Ok(Behaviors::unhandled()),
  })
}

fn apply_event(state: &u32, event: &u32) -> u32 {
  state + event
}

fn message_adapter() -> PersistenceEffectorMessageAdapter<u32, u32, AggregateCommand> {
  PersistenceEffectorMessageAdapter::new(AggregateCommand::Signal, |message| match message {
    | AggregateCommand::Signal(signal) => Some(signal),
    | AggregateCommand::Add(_) => None,
  })
}

fn typed_persistence_system(props: &TypedProps<AggregateCommand>) -> TypedActorSystem<AggregateCommand> {
  let persistence = PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let installers = ExtensionInstallers::default().with_extension_installer(persistence);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers);
  TypedActorSystem::<AggregateCommand>::create_from_props(props, config).expect("typed system should start")
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  let deadline = Instant::now() + Duration::from_millis(5000);
  while Instant::now() < deadline {
    if condition() {
      return;
    }
    thread::yield_now();
  }
  assert!(condition(), "timed out waiting for persistence effector test condition");
}

fn contains_published_event(
  recorded_events: &ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
  persistence_id: &str,
  sequence_nr: u64,
  event: u32,
) -> bool {
  recorded_events.lock().iter().any(|stream_event| {
    let EventStreamEvent::Extension { name, payload } = stream_event else {
      return false;
    };
    name == "persistence"
      && payload.downcast_ref::<PublishedEvent<u32>>().is_some_and(|published| {
        published.persistence_id().as_str() == persistence_id
          && published.sequence_nr() == sequence_nr
          && *published.event() == event
          && published.tags() == &BTreeSet::new()
      })
  })
}
