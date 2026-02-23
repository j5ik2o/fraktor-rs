use alloc::{string::ToString, vec::Vec};

use fraktor_actor_rs::core::{
  actor::{
    Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::SendError,
  messaging::AnyMessageGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::ArcShared,
};

use crate::core::{
  journal_message::JournalMessage, persistence_context::PersistenceContext, snapshot_message::SnapshotMessage,
};

type TB = NoStdToolbox;
type MessageStore = ArcShared<ToolboxMutex<Vec<AnyMessageGeneric<TB>>, TB>>;

struct DummyActor;

type DummyContext = PersistenceContext<DummyActor, TB>;

struct TestSender {
  messages: MessageStore,
}

impl ActorRefSender<TB> for TestSender {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

fn create_sender() -> (ActorRefGeneric<TB>, MessageStore) {
  let messages = ArcShared::new(<<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()));
  let sender = ActorRefGeneric::new(Pid::new(1, 1), TestSender { messages: messages.clone() });
  (sender, messages)
}

#[test]
fn context_sends_journal_messages() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");

  let message = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".to_string(),
    to_sequence_nr: 10,
    sender:         ActorRefGeneric::null(),
  };
  context.send_write_messages(message).expect("send");

  let messages = journal_store.lock();
  assert_eq!(messages.len(), 1);
}

#[test]
fn context_sends_snapshot_messages() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");

  let message = SnapshotMessage::DeleteSnapshots {
    persistence_id: "pid-1".to_string(),
    criteria:       crate::core::snapshot_selection_criteria::SnapshotSelectionCriteria::latest(),
    sender:         ActorRefGeneric::null(),
  };
  context.send_snapshot_message(message).expect("send");

  let messages = snapshot_store.lock();
  assert_eq!(messages.len(), 1);
}

#[test]
fn bind_actor_refs_rejects_second_bind() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());

  assert!(context.bind_actor_refs(journal_ref, snapshot_ref).is_ok());
  let result = context.bind_actor_refs(ActorRefGeneric::null(), ActorRefGeneric::null());
  assert!(result.is_err());
}

/// Regression test for GitHub Issue #125: `is_bound` must use OR semantics
/// (either ref non-null → bound) while `is_ready` uses AND semantics
/// (both refs non-null → ready). PR #117 accidentally changed `is_bound`
/// to AND, making it identical to `is_ready`.
#[test]
fn is_bound_returns_true_when_only_journal_ref_is_set() {
  // Given: a context with only journal_actor_ref set (partial binding)
  let mut context = DummyContext::new("pid-1".to_string());
  let (journal_ref, _) = create_sender();
  context.journal_actor_ref = journal_ref;

  // Then: is_bound is true (OR: at least one ref is set)
  assert!(context.is_bound());
  // Then: is_ready is false (AND: both refs must be set)
  assert!(!context.is_ready());
}

/// Regression test for GitHub Issue #125: partial binding with only
/// snapshot_actor_ref must also be considered "bound".
#[test]
fn is_bound_returns_true_when_only_snapshot_ref_is_set() {
  // Given: a context with only snapshot_actor_ref set (partial binding)
  let mut context = DummyContext::new("pid-1".to_string());
  let (snapshot_ref, _) = create_sender();
  context.snapshot_actor_ref = snapshot_ref;

  // Then: is_bound is true (OR: at least one ref is set)
  assert!(context.is_bound());
  // Then: is_ready is false (AND: both refs must be set)
  assert!(!context.is_ready());
}

#[test]
fn is_bound_returns_false_when_neither_ref_is_set() {
  // Given: a freshly created context (both refs are null)
  let context = DummyContext::new("pid-1".to_string());

  // Then: neither bound nor ready
  assert!(!context.is_bound());
  assert!(!context.is_ready());
}

#[test]
fn start_recovery_none_requests_highest_sequence_nr() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");

  context.start_recovery(crate::core::Recovery::none(), ActorRefGeneric::null()).expect("start recovery");

  let journal_messages = journal_store.lock();
  assert_eq!(journal_messages.len(), 1);
  let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
  match message {
    | JournalMessage::GetHighestSequenceNr { persistence_id, from_sequence_nr, .. } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(*from_sequence_nr, 0);
    },
    | _ => panic!("unexpected message"),
  }

  let snapshot_messages = snapshot_store.lock();
  assert_eq!(snapshot_messages.len(), 0);
}
