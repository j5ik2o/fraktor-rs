use alloc::vec;

use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorContextGeneric, Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::SendError,
  messaging::AnyMessageGeneric,
  system::{ActorSystemGeneric, SystemStateGeneric, SystemStateSharedGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use crate::core::{
  in_memory_journal::InMemoryJournal, journal_actor::JournalActor, journal_message::JournalMessage,
  journal_response::JournalResponse, persistent_repr::PersistentRepr,
};

type TB = NoStdToolbox;
type MessageStore = ArcShared<ToolboxMutex<Vec<AnyMessageGeneric<TB>>, TB>>;

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
  let messages = ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()));
  let sender = ActorRefGeneric::new(Pid::new(1, 1), TestSender { messages: messages.clone() });
  (sender, messages)
}

fn new_test_system() -> ActorSystemGeneric<TB> {
  let state = SystemStateGeneric::new();
  let state = SystemStateSharedGeneric::new(state);
  state.mark_root_started();
  ActorSystemGeneric::from_state(state)
}

#[test]
fn journal_actor_write_messages_sends_responses() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let mut actor = JournalActor::<InMemoryJournal, TB>::new(InMemoryJournal::new());
  let (sender, store) = create_sender();

  let payload1 = ArcShared::new(1_i32);
  let payload2 = ArcShared::new(2_i32);
  let repr1 = PersistentRepr::new("pid-1", 1, payload1);
  let repr2 = PersistentRepr::new("pid-1", 2, payload2);
  let message = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 2,
    messages: vec![repr1, repr2],
    sender,
    instance_id: 9,
  };

  let any_message = AnyMessageGeneric::new(message);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 3);
  let mut success_count = 0;
  let mut batch_success = 0;
  for response in responses.iter() {
    let response = response.payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
    match response {
      | JournalResponse::WriteMessageSuccess { .. } => success_count += 1,
      | JournalResponse::WriteMessagesSuccessful => batch_success += 1,
      | _ => {},
    }
  }
  assert_eq!(success_count, 2);
  assert_eq!(batch_success, 1);
}
