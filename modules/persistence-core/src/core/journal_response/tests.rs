use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{journal_error::JournalError, journal_response::JournalResponse, persistent_repr::PersistentRepr};

fn repr(sequence_nr: u64) -> PersistentRepr {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(sequence_nr as i32);
  PersistentRepr::new("pid-1", sequence_nr, payload)
}

#[test]
fn journal_response_variants_hold_data() {
  let repr1 = repr(1);
  let repr2 = repr(2);

  let success = JournalResponse::WriteMessageSuccess { repr: repr1.clone(), instance_id: 5 };
  let failure = JournalResponse::WriteMessageFailure {
    repr:        repr1.clone(),
    cause:       JournalError::ReadFailed("io".into()),
    instance_id: 6,
  };
  let rejected = JournalResponse::WriteMessageRejected {
    repr:        repr1.clone(),
    cause:       JournalError::WriteFailed("reject".into()),
    instance_id: 7,
  };
  let bulk_ok = JournalResponse::WriteMessagesSuccessful { instance_id: 8 };
  let bulk_failed = JournalResponse::WriteMessagesFailed {
    cause:       JournalError::DeleteFailed("oops".into()),
    write_count: 2,
    instance_id: 9,
  };
  let replayed = JournalResponse::ReplayedMessage { persistent_repr: repr2.clone() };
  let recovery = JournalResponse::RecoverySuccess { highest_sequence_nr: 9 };
  let highest = JournalResponse::HighestSequenceNr { persistence_id: "pid-1".into(), sequence_nr: 9 };
  let highest_failed = JournalResponse::HighestSequenceNrFailure {
    persistence_id: "pid-1".into(),
    cause:          JournalError::ReadFailed("highest".into()),
  };
  let replay_failed = JournalResponse::ReplayMessagesFailure { cause: JournalError::ReadFailed("bad".into()) };
  let delete_ok = JournalResponse::DeleteMessagesSuccess { to_sequence_nr: 3 };
  let delete_failed = JournalResponse::DeleteMessagesFailure {
    cause:          JournalError::DeleteFailed("fail".into()),
    to_sequence_nr: 4,
  };

  match success {
    | JournalResponse::WriteMessageSuccess { repr, instance_id } => {
      assert_eq!(repr.sequence_nr(), 1);
      assert_eq!(instance_id, 5);
    },
    | _ => panic!("unexpected variant"),
  }

  match failure {
    | JournalResponse::WriteMessageFailure { repr, instance_id, .. } => {
      assert_eq!(repr.sequence_nr(), 1);
      assert_eq!(instance_id, 6);
    },
    | _ => panic!("unexpected variant"),
  }

  match rejected {
    | JournalResponse::WriteMessageRejected { repr, instance_id, .. } => {
      assert_eq!(repr.sequence_nr(), 1);
      assert_eq!(instance_id, 7);
    },
    | _ => panic!("unexpected variant"),
  }

  match bulk_ok {
    | JournalResponse::WriteMessagesSuccessful { instance_id } => assert_eq!(instance_id, 8),
    | _ => panic!("unexpected variant"),
  }

  match bulk_failed {
    | JournalResponse::WriteMessagesFailed { write_count, instance_id, .. } => {
      assert_eq!(write_count, 2);
      assert_eq!(instance_id, 9);
    },
    | _ => panic!("unexpected variant"),
  }

  match replayed {
    | JournalResponse::ReplayedMessage { persistent_repr } => assert_eq!(persistent_repr.sequence_nr(), 2),
    | _ => panic!("unexpected variant"),
  }

  match recovery {
    | JournalResponse::RecoverySuccess { highest_sequence_nr } => assert_eq!(highest_sequence_nr, 9),
    | _ => panic!("unexpected variant"),
  }

  match highest {
    | JournalResponse::HighestSequenceNr { sequence_nr, .. } => assert_eq!(sequence_nr, 9),
    | _ => panic!("unexpected variant"),
  }

  match highest_failed {
    | JournalResponse::HighestSequenceNrFailure { cause, .. } => {
      assert_eq!(cause, JournalError::ReadFailed("highest".into()));
    },
    | _ => panic!("unexpected variant"),
  }

  match replay_failed {
    | JournalResponse::ReplayMessagesFailure { .. } => {},
    | _ => panic!("unexpected variant"),
  }

  match delete_ok {
    | JournalResponse::DeleteMessagesSuccess { to_sequence_nr } => assert_eq!(to_sequence_nr, 3),
    | _ => panic!("unexpected variant"),
  }

  match delete_failed {
    | JournalResponse::DeleteMessagesFailure { to_sequence_nr, .. } => assert_eq!(to_sequence_nr, 4),
    | _ => panic!("unexpected variant"),
  }
}
