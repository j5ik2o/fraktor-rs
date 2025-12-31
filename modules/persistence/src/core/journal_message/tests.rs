use alloc::vec;

use fraktor_actor_rs::core::actor::actor_ref::ActorRef;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{journal_message::JournalMessage, persistent_repr::PersistentRepr};

#[test]
fn journal_message_write_messages_fields() {
  let sender = ActorRef::null();
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, payload);

  let message = JournalMessage::<NoStdToolbox>::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    messages:       vec![repr.clone()],
    sender:         sender.clone(),
    instance_id:    7,
  };

  match message {
    | JournalMessage::WriteMessages { persistence_id, to_sequence_nr, messages, sender: _, instance_id } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(to_sequence_nr, 1);
      assert_eq!(messages.len(), 1);
      assert_eq!(messages[0].sequence_nr(), repr.sequence_nr());
      assert_eq!(instance_id, 7);
    },
    | _ => panic!("unexpected variant"),
  }
}

#[test]
fn journal_message_replay_fields() {
  let sender = ActorRef::null();
  let message = JournalMessage::<NoStdToolbox>::ReplayMessages {
    persistence_id: "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr: 5,
    max: 10,
    sender,
  };

  match message {
    | JournalMessage::ReplayMessages { persistence_id, from_sequence_nr, to_sequence_nr, max, sender: _ } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(from_sequence_nr, 1);
      assert_eq!(to_sequence_nr, 5);
      assert_eq!(max, 10);
    },
    | _ => panic!("unexpected variant"),
  }
}

#[test]
fn journal_message_delete_and_highest_fields() {
  let sender = ActorRef::null();
  let delete_message = JournalMessage::<NoStdToolbox>::DeleteMessagesTo {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 3,
    sender:         sender.clone(),
  };
  let highest_message = JournalMessage::<NoStdToolbox>::GetHighestSequenceNr {
    persistence_id: "pid-1".into(),
    from_sequence_nr: 1,
    sender,
  };

  match delete_message {
    | JournalMessage::DeleteMessagesTo { persistence_id, to_sequence_nr, sender: _ } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(to_sequence_nr, 3);
    },
    | _ => panic!("unexpected variant"),
  }

  match highest_message {
    | JournalMessage::GetHighestSequenceNr { persistence_id, from_sequence_nr, sender: _ } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(from_sequence_nr, 1);
    },
    | _ => panic!("unexpected variant"),
  }
}
