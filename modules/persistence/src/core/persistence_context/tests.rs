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
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
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
