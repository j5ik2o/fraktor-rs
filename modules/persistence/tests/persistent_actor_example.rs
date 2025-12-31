//! Persistent actor base flow example.

use core::{
  future::Future,
  mem::take,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use fraktor_actor_rs::core::{
  actor::{
    ActorContextGeneric, Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
};
use fraktor_persistence_rs::core::{
  Eventsourced, InMemoryJournal, InMemorySnapshotStore, Journal, JournalError, JournalMessage, JournalResponse,
  PersistentActor, PersistentActorBase, PersistentRepr, Snapshot, SnapshotMessage, SnapshotResponse, SnapshotStore,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

#[derive(Clone, Debug)]
enum CounterEvent {
  Incremented(i64),
  Decremented(i64),
}

struct CounterActor {
  base:    PersistentActorBase<Self, NoStdToolbox>,
  value:   i64,
  applied: Vec<CounterEvent>,
}

impl CounterActor {
  fn new(
    persistence_id: &str,
    journal_actor_ref: ActorRefGeneric<NoStdToolbox>,
    snapshot_actor_ref: ActorRefGeneric<NoStdToolbox>,
  ) -> Self {
    Self {
      base:    PersistentActorBase::new(persistence_id.to_string(), journal_actor_ref, snapshot_actor_ref),
      value:   0,
      applied: Vec::new(),
    }
  }

  fn enqueue_event(&mut self, event: CounterEvent, stashing: bool) {
    let handler = Box::new(|actor: &mut Self, repr: &PersistentRepr| {
      if let Some(event) = repr.downcast_ref::<CounterEvent>() {
        actor.apply_event(event);
      }
    });
    self.base.add_to_event_batch(event, stashing, handler);
  }

  fn apply_event(&mut self, event: &CounterEvent) {
    match event {
      | CounterEvent::Incremented(amount) => self.value += amount,
      | CounterEvent::Decremented(amount) => self.value -= amount,
    }
    self.applied.push(event.clone());
  }
}

impl Eventsourced<NoStdToolbox> for CounterActor {
  fn persistence_id(&self) -> &str {
    self.base.persistence_id()
  }

  fn journal_actor_ref(&self) -> &ActorRefGeneric<NoStdToolbox> {
    self.base.journal_actor_ref()
  }

  fn snapshot_actor_ref(&self) -> &ActorRefGeneric<NoStdToolbox> {
    self.base.snapshot_actor_ref()
  }

  fn receive_recover(&mut self, repr: &PersistentRepr) {
    if let Some(event) = repr.downcast_ref::<CounterEvent>() {
      self.apply_event(event);
    }
  }

  fn receive_snapshot(&mut self, snapshot: &Snapshot) {
    if let Some(value) = snapshot.data().downcast_ref::<i64>() {
      self.value = *value;
    }
  }

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.base.last_sequence_nr()
  }
}

impl PersistentActor<NoStdToolbox> for CounterActor {
  fn base(&self) -> &PersistentActorBase<Self, NoStdToolbox> {
    &self.base
  }

  fn base_mut(&mut self) -> &mut PersistentActorBase<Self, NoStdToolbox> {
    &mut self.base
  }
}

struct TestRuntime {
  journal:            InMemoryJournal,
  snapshot_store:     InMemorySnapshotStore,
  journal_responses:  Vec<JournalResponse>,
  snapshot_responses: Vec<SnapshotResponse>,
}

impl TestRuntime {
  fn new() -> Self {
    Self {
      journal:            InMemoryJournal::new(),
      snapshot_store:     InMemorySnapshotStore::new(),
      journal_responses:  Vec::new(),
      snapshot_responses: Vec::new(),
    }
  }

  fn take_journal_responses(&mut self) -> Vec<JournalResponse> {
    take(&mut self.journal_responses)
  }
}

struct JournalSender {
  runtime: ArcShared<ToolboxMutex<TestRuntime, NoStdToolbox>>,
}

impl JournalSender {
  fn new(runtime: ArcShared<ToolboxMutex<TestRuntime, NoStdToolbox>>) -> Self {
    Self { runtime }
  }
}

impl ActorRefSender<NoStdToolbox> for JournalSender {
  fn send(&mut self, message: AnyMessageGeneric<NoStdToolbox>) -> Result<SendOutcome, SendError<NoStdToolbox>> {
    if let Some(journal_message) = message.payload().downcast_ref::<JournalMessage<NoStdToolbox>>().cloned() {
      let mut runtime = self.runtime.lock();
      let responses = handle_journal_message(&mut runtime, journal_message);
      runtime.journal_responses.extend(responses);
    }
    Ok(SendOutcome::Delivered)
  }
}

struct SnapshotSender {
  runtime: ArcShared<ToolboxMutex<TestRuntime, NoStdToolbox>>,
}

impl SnapshotSender {
  fn new(runtime: ArcShared<ToolboxMutex<TestRuntime, NoStdToolbox>>) -> Self {
    Self { runtime }
  }
}

impl ActorRefSender<NoStdToolbox> for SnapshotSender {
  fn send(&mut self, message: AnyMessageGeneric<NoStdToolbox>) -> Result<SendOutcome, SendError<NoStdToolbox>> {
    if let Some(snapshot_message) = message.payload().downcast_ref::<SnapshotMessage<NoStdToolbox>>().cloned() {
      let mut runtime = self.runtime.lock();
      let responses = handle_snapshot_message(&mut runtime, snapshot_message);
      runtime.snapshot_responses.extend(responses);
    }
    Ok(SendOutcome::Delivered)
  }
}

fn handle_journal_message(runtime: &mut TestRuntime, message: JournalMessage<NoStdToolbox>) -> Vec<JournalResponse> {
  match message {
    | JournalMessage::WriteMessages { messages, instance_id, .. } => {
      let write_count = messages.len() as u64;
      let result = drive_ready(runtime.journal.write_messages(&messages));
      let mut responses = Vec::new();
      match result {
        | Ok(()) => {
          for repr in messages {
            responses.push(JournalResponse::WriteMessageSuccess { repr, instance_id });
          }
          responses.push(JournalResponse::WriteMessagesSuccessful);
        },
        | Err(error) => {
          let rejected = matches!(error, JournalError::SequenceMismatch { .. });
          for repr in messages {
            if rejected {
              responses.push(JournalResponse::WriteMessageRejected { repr, cause: error.clone(), instance_id });
            } else {
              responses.push(JournalResponse::WriteMessageFailure { repr, cause: error.clone(), instance_id });
            }
          }
          responses.push(JournalResponse::WriteMessagesFailed { cause: error, write_count });
        },
      }
      responses
    },
    | JournalMessage::ReplayMessages { persistence_id, from_sequence_nr, to_sequence_nr, max, .. } => {
      let mut responses = Vec::new();
      match drive_ready(runtime.journal.replay_messages(&persistence_id, from_sequence_nr, to_sequence_nr, max)) {
        | Ok(replayed) => {
          for repr in replayed {
            responses.push(JournalResponse::ReplayedMessage { persistent_repr: repr });
          }
          match drive_ready(runtime.journal.highest_sequence_nr(&persistence_id)) {
            | Ok(highest) => responses.push(JournalResponse::RecoverySuccess { highest_sequence_nr: highest }),
            | Err(error) => responses.push(JournalResponse::ReplayMessagesFailure { cause: error }),
          }
        },
        | Err(error) => responses.push(JournalResponse::ReplayMessagesFailure { cause: error }),
      }
      responses
    },
    | JournalMessage::DeleteMessagesTo { persistence_id, to_sequence_nr, .. } => {
      match drive_ready(runtime.journal.delete_messages_to(&persistence_id, to_sequence_nr)) {
        | Ok(()) => vec![JournalResponse::DeleteMessagesSuccess { to_sequence_nr }],
        | Err(error) => vec![JournalResponse::DeleteMessagesFailure { cause: error, to_sequence_nr }],
      }
    },
    | JournalMessage::GetHighestSequenceNr { persistence_id, .. } => {
      match drive_ready(runtime.journal.highest_sequence_nr(&persistence_id)) {
        | Ok(sequence_nr) => vec![JournalResponse::HighestSequenceNr { persistence_id, sequence_nr }],
        | Err(error) => vec![JournalResponse::ReplayMessagesFailure { cause: error }],
      }
    },
  }
}

fn handle_snapshot_message(runtime: &mut TestRuntime, message: SnapshotMessage<NoStdToolbox>) -> Vec<SnapshotResponse> {
  match message {
    | SnapshotMessage::SaveSnapshot { metadata, snapshot, .. } => {
      match drive_ready(runtime.snapshot_store.save_snapshot(metadata.clone(), snapshot)) {
        | Ok(()) => vec![SnapshotResponse::SaveSnapshotSuccess { metadata }],
        | Err(error) => vec![SnapshotResponse::SaveSnapshotFailure { metadata, error }],
      }
    },
    | SnapshotMessage::LoadSnapshot { persistence_id, criteria, .. } => {
      match drive_ready(runtime.snapshot_store.load_snapshot(&persistence_id, criteria)) {
        | Ok(snapshot) => {
          let to_sequence_nr = snapshot.as_ref().map(|value| value.metadata().sequence_nr()).unwrap_or(0);
          vec![SnapshotResponse::LoadSnapshotResult { snapshot, to_sequence_nr }]
        },
        | Err(error) => vec![SnapshotResponse::LoadSnapshotFailed { error }],
      }
    },
    | SnapshotMessage::DeleteSnapshot { metadata, .. } => {
      match drive_ready(runtime.snapshot_store.delete_snapshot(&metadata)) {
        | Ok(()) => vec![SnapshotResponse::DeleteSnapshotSuccess { metadata }],
        | Err(error) => vec![SnapshotResponse::DeleteSnapshotFailure { metadata, error }],
      }
    },
    | SnapshotMessage::DeleteSnapshots { persistence_id, criteria, .. } => {
      let criteria_clone = criteria.clone();
      match drive_ready(runtime.snapshot_store.delete_snapshots(&persistence_id, criteria_clone)) {
        | Ok(()) => vec![SnapshotResponse::DeleteSnapshotsSuccess { criteria }],
        | Err(error) => vec![SnapshotResponse::DeleteSnapshotsFailure { criteria, error }],
      }
    },
  }
}

fn drive_ready<F: Future>(future: F) -> F::Output {
  let waker = unsafe { Waker::from_raw(raw_waker()) };
  let mut context = Context::from_waker(&waker);
  let mut future = core::pin::pin!(future);
  match Future::poll(future.as_mut(), &mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was not ready"),
  }
}

fn raw_waker() -> RawWaker {
  RawWaker::new(core::ptr::null(), &RAW_WAKER_VTABLE)
}

static RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(clone_raw, wake_raw, wake_raw, drop_raw);

unsafe fn clone_raw(_: *const ()) -> RawWaker {
  raw_waker()
}

const unsafe fn wake_raw(_: *const ()) {}

const unsafe fn drop_raw(_: *const ()) {}

fn build_runtime() -> ArcShared<ToolboxMutex<TestRuntime, NoStdToolbox>> {
  ArcShared::new(<ToolboxMutex<_, NoStdToolbox> as SyncMutexLike<_>>::new(TestRuntime::new()))
}

fn build_journal_ref(runtime: ArcShared<ToolboxMutex<TestRuntime, NoStdToolbox>>) -> ActorRefGeneric<NoStdToolbox> {
  ActorRefGeneric::new(Pid::new(1, 0), JournalSender::new(runtime))
}

fn build_snapshot_ref(runtime: ArcShared<ToolboxMutex<TestRuntime, NoStdToolbox>>) -> ActorRefGeneric<NoStdToolbox> {
  ActorRefGeneric::new(Pid::new(2, 0), SnapshotSender::new(runtime))
}

#[test]
fn test_persistence_batch_flow() {
  let runtime = build_runtime();
  let journal_ref = build_journal_ref(runtime.clone());
  let snapshot_ref = build_snapshot_ref(runtime.clone());
  let mut actor = CounterActor::new("counter-1", journal_ref, snapshot_ref);

  actor.enqueue_event(CounterEvent::Incremented(10), true);
  actor.enqueue_event(CounterEvent::Decremented(3), true);
  actor.base_mut().flush_batch(ActorRefGeneric::null());

  let responses = runtime.lock().take_journal_responses();
  for response in responses {
    actor.handle_journal_response(&response);
  }

  assert_eq!(actor.value, 7);
  assert_eq!(actor.applied.len(), 2);
  assert_eq!(actor.last_sequence_nr(), 2);
}

#[test]
fn test_replay_messages_updates_state() {
  let runtime = build_runtime();
  let journal_ref = build_journal_ref(runtime.clone());
  let snapshot_ref = build_snapshot_ref(runtime.clone());

  let mut actor1 = CounterActor::new("counter-1", journal_ref.clone(), snapshot_ref.clone());
  actor1.enqueue_event(CounterEvent::Incremented(10), true);
  actor1.enqueue_event(CounterEvent::Incremented(5), true);
  actor1.enqueue_event(CounterEvent::Decremented(2), true);
  actor1.base_mut().flush_batch(ActorRefGeneric::null());

  let responses = runtime.lock().take_journal_responses();
  for response in responses {
    actor1.handle_journal_response(&response);
  }

  let replay = JournalMessage::<NoStdToolbox>::ReplayMessages {
    persistence_id:   "counter-1".to_string(),
    from_sequence_nr: 1,
    to_sequence_nr:   100,
    max:              100,
    sender:           ActorRefGeneric::null(),
  };
  let _ = journal_ref.tell(AnyMessageGeneric::new(replay));

  let mut actor2 = CounterActor::new("counter-1", journal_ref, snapshot_ref);
  let responses = runtime.lock().take_journal_responses();
  for response in responses {
    actor2.handle_journal_response(&response);
  }

  assert_eq!(actor2.value, 13);
  assert_eq!(actor2.last_sequence_nr(), 3);
}
