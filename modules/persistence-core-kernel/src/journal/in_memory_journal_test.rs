use alloc::{boxed::Box, vec::Vec};
use core::{
  any::Any,
  future::{Future, Ready, ready},
  task::{Context, Poll, Waker},
};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  journal::{InMemoryJournal, Journal, JournalError},
  persistent::{AtomicWrite, PersistentRepr},
};

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::noop();
  let mut cx = Context::from_waker(waker);
  let mut future = Box::pin(future);
  match Future::poll(future.as_mut(), &mut cx) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was pending"),
  }
}

fn build_messages(persistence_id: &str, start: u64, count: u64) -> Vec<PersistentRepr> {
  (0..count)
    .map(|offset| {
      let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new((start + offset) as i32);
      PersistentRepr::new(persistence_id, start + offset, payload)
    })
    .collect()
}

fn atomic_write(payload: Vec<PersistentRepr>) -> AtomicWrite {
  AtomicWrite::new(payload).expect("atomic write")
}

#[derive(Default)]
struct SingleEntryOnlyJournal {
  persisted: Vec<PersistentRepr>,
}

impl Journal for SingleEntryOnlyJournal {
  type DeleteFuture<'a>
    = Ready<Result<(), JournalError>>
  where
    Self: 'a;
  type HighestSeqNrFuture<'a>
    = Ready<Result<u64, JournalError>>
  where
    Self: 'a;
  type ReplayFuture<'a>
    = Ready<Result<Vec<PersistentRepr>, JournalError>>
  where
    Self: 'a;
  type WriteFuture<'a>
    = Ready<Result<(), JournalError>>
  where
    Self: 'a;

  fn write_messages<'a>(&'a mut self, messages: &'a [AtomicWrite]) -> Self::WriteFuture<'a> {
    for message in messages {
      if message.size() > 1 {
        return ready(Err(JournalError::UnsupportedAtomicWrite { size: message.size() }));
      }
    }
    for message in messages {
      self.persisted.extend(message.payload().iter().cloned());
    }
    ready(Ok(()))
  }

  fn replay_messages<'a>(
    &'a self,
    _persistence_id: &'a str,
    _from_sequence_nr: u64,
    _to_sequence_nr: u64,
    _max: u64,
  ) -> Self::ReplayFuture<'a> {
    ready(Ok(self.persisted.clone()))
  }

  fn delete_messages_to<'a>(&'a mut self, _persistence_id: &'a str, _to_sequence_nr: u64) -> Self::DeleteFuture<'a> {
    ready(Ok(()))
  }

  fn highest_sequence_nr<'a>(&'a self, _persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a> {
    ready(Ok(0))
  }
}

#[test]
fn in_memory_journal_write_and_replay() {
  let mut journal = InMemoryJournal::new();
  let messages = build_messages("pid-1", 1, 3);

  let result = poll_ready(journal.write_messages(&[atomic_write(messages)]));
  assert!(result.is_ok());

  let replayed = poll_ready(journal.replay_messages("pid-1", 1, 3, 10)).expect("replay failed");
  assert_eq!(replayed.len(), 3);
  assert_eq!(replayed[0].sequence_nr(), 1);
  assert_eq!(replayed[1].sequence_nr(), 2);
  assert_eq!(replayed[2].sequence_nr(), 3);
}

#[test]
fn in_memory_journal_sequence_mismatch() {
  let mut journal = InMemoryJournal::new();
  let messages = build_messages("pid-1", 2, 1);

  let result = poll_ready(journal.write_messages(&[atomic_write(messages)]));
  assert_eq!(result, Err(JournalError::SequenceMismatch { expected: 1, actual: 2 }));
}

#[test]
fn in_memory_journal_replay_respects_max() {
  let mut journal = InMemoryJournal::new();
  let messages = build_messages("pid-1", 1, 3);
  poll_ready(journal.write_messages(&[atomic_write(messages)])).expect("write failed");

  let replayed = poll_ready(journal.replay_messages("pid-1", 1, 3, 1)).expect("replay failed");
  assert_eq!(replayed.len(), 1);
  assert_eq!(replayed[0].sequence_nr(), 1);
}

#[test]
fn in_memory_journal_delete_messages_to() {
  let mut journal = InMemoryJournal::new();
  let messages = build_messages("pid-1", 1, 3);
  poll_ready(journal.write_messages(&[atomic_write(messages)])).expect("write failed");

  poll_ready(journal.delete_messages_to("pid-1", 2)).expect("delete failed");

  let replayed = poll_ready(journal.replay_messages("pid-1", 1, 3, 10)).expect("replay failed");
  assert_eq!(replayed.len(), 1);
  assert_eq!(replayed[0].sequence_nr(), 3);
}

#[test]
fn in_memory_journal_delete_keeps_highest_sequence_nr() {
  let mut journal = InMemoryJournal::new();
  let messages = build_messages("pid-1", 1, 3);
  poll_ready(journal.write_messages(&[atomic_write(messages)])).expect("write failed");

  poll_ready(journal.delete_messages_to("pid-1", 3)).expect("delete failed");

  let highest = poll_ready(journal.highest_sequence_nr("pid-1")).expect("highest failed");
  assert_eq!(highest, 3);

  let mismatch = build_messages("pid-1", 1, 1);
  let result = poll_ready(journal.write_messages(&[atomic_write(mismatch)]));
  assert_eq!(result, Err(JournalError::SequenceMismatch { expected: 4, actual: 1 }));

  let next = build_messages("pid-1", 4, 1);
  poll_ready(journal.write_messages(&[atomic_write(next)])).expect("write failed");
}

#[test]
fn in_memory_journal_highest_sequence_nr_defaults_to_zero() {
  let journal = InMemoryJournal::new();

  let highest = poll_ready(journal.highest_sequence_nr("missing")).expect("highest failed");
  assert_eq!(highest, 0);
}

#[test]
fn backend_rejects_unsupported_multi_entry_atomic_write_without_partial_persistence() {
  let mut journal = SingleEntryOnlyJournal::default();
  let multi_entry = atomic_write(build_messages("pid-1", 1, 2));

  let result = poll_ready(journal.write_messages(&[multi_entry]));

  assert_eq!(result, Err(JournalError::UnsupportedAtomicWrite { size: 2 }));
  assert!(journal.persisted.is_empty());
}
