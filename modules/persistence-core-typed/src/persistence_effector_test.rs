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
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::{
  EventRejectedError, EventSourcedSignal, PersistenceEffector, PersistenceEffectorConfig,
  PersistenceEffectorMessageAdapter, PersistenceEffectorSignal, PersistenceId, PersistenceMode, PublishedEvent,
  RetentionCriteria, persistence_effector_signal_auth::PersistenceEffectorSignalAuth,
};

type RecordedEvents = SharedLock<Vec<EventStreamEvent>>;
type PersistedEvents = SharedLock<Vec<u32>>;

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
  let recorded_events = new_recorded_events();
  let persisted_events = new_persisted_events();
  let props = aggregate_props("published-event", persisted_events.clone(), true);
  let system = typed_persistence_system(&props);
  let subscriber = subscriber_handle(RecordingSubscriber::new(recorded_events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);
  let mut guardian = system.user_guardian_ref();
  guardian.try_tell(AggregateCommand::Add(7)).expect("persist command should be accepted");
  wait_for_persisted_events(&persisted_events, &[7]);
  wait_until(|| contains_published_event(&recorded_events, "published-event", 1, 7));

  system.terminate().expect("terminate");
}

#[test]
fn event_publishing_enabled_publishes_ephemeral_event_to_system_event_stream() {
  let recorded_events = new_recorded_events();
  let persisted_events = new_persisted_events();
  let props =
    aggregate_props_with_mode("ephemeral-published-event", persisted_events.clone(), true, PersistenceMode::Ephemeral);
  let system = typed_persistence_system(&props);
  let subscriber = subscriber_handle(RecordingSubscriber::new(recorded_events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);
  let mut guardian = system.user_guardian_ref();
  guardian.try_tell(AggregateCommand::Add(11)).expect("persist command should be accepted");
  wait_for_persisted_events(&persisted_events, &[11]);
  wait_until(|| contains_published_event(&recorded_events, "ephemeral-published-event", 1, 11));

  system.terminate().expect("terminate");
}

#[test]
fn event_publishing_disabled_does_not_publish_persisted_event_to_system_event_stream() {
  let recorded_events = new_recorded_events();
  let persisted_events = new_persisted_events();
  let props = aggregate_props("unpublished-event", persisted_events.clone(), false);
  let system = typed_persistence_system(&props);
  let subscriber = subscriber_handle(RecordingSubscriber::new(recorded_events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);
  let mut guardian = system.user_guardian_ref();
  guardian.try_tell(AggregateCommand::Add(9)).expect("persist command should be accepted");
  wait_for_persisted_events(&persisted_events, &[9]);
  assert_never_for(Duration::from_millis(200), || {
    contains_published_event(&recorded_events, "unpublished-event", 1, 9)
  });

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
    auth:   PersistenceEffectorSignalAuth::new(),
    signal: EventSourcedSignal::JournalPersistRejected { error: error.clone() },
  };
  assert!(
    matches!(
      signal,
      PersistenceEffectorSignal::EventSourced {
        auth: _,
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
  events: RecordedEvents,
}

impl RecordingSubscriber {
  const fn new(events: RecordedEvents) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    record_event(&self.events, event);
  }
}

fn aggregate_props(
  persistence_id: &str,
  persisted_events: PersistedEvents,
  event_publishing: bool,
) -> TypedProps<AggregateCommand> {
  aggregate_props_with_mode(persistence_id, persisted_events, event_publishing, PersistenceMode::Persisted)
}

fn aggregate_props_with_mode(
  persistence_id: &str,
  persisted_events: PersistedEvents,
  event_publishing: bool,
  persistence_mode: PersistenceMode,
) -> TypedProps<AggregateCommand> {
  let config = PersistenceEffectorConfig::new(PersistenceId::of_unique_id(persistence_id), 0_u32, apply_event)
    .with_message_adapter(message_adapter())
    .with_event_publishing(event_publishing)
    .with_persistence_mode(persistence_mode);

  PersistenceEffector::props(config, move |_state, effector| Ok(aggregate_behavior(effector, persisted_events.clone())))
}

fn aggregate_behavior(
  effector: PersistenceEffector<u32, u32, AggregateCommand>,
  persisted_events: PersistedEvents,
) -> Behavior<AggregateCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | AggregateCommand::Add(value) => {
      let persisted_events = persisted_events.clone();
      effector.persist_event(ctx, *value, move |event| {
        record_persisted_event(&persisted_events, *event);
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

fn assert_never_for(duration: Duration, mut condition: impl FnMut() -> bool) {
  let deadline = Instant::now() + duration;
  while Instant::now() < deadline {
    assert!(!condition(), "unexpected event publication observed");
    thread::yield_now();
  }
}

fn new_recorded_events() -> RecordedEvents {
  SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new())
}

fn new_persisted_events() -> PersistedEvents {
  SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new())
}

fn record_event(recorded_events: &RecordedEvents, event: &EventStreamEvent) {
  recorded_events.with_write(|events| events.push(event.clone()));
}

fn record_persisted_event(persisted_events: &PersistedEvents, event: u32) {
  persisted_events.with_write(|events| events.push(event));
}

fn wait_for_persisted_events(persisted_events: &PersistedEvents, expected: &[u32]) {
  wait_until(|| persisted_events.with_read(|events| events.as_slice() == expected));
}

fn contains_published_event(
  recorded_events: &RecordedEvents,
  persistence_id: &str,
  sequence_nr: u64,
  event: u32,
) -> bool {
  recorded_events.with_read(|events| {
    events.iter().any(|stream_event| {
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
  })
}
