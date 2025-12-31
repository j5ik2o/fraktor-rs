use alloc::{boxed::Box, vec::Vec};
use core::{
  future::Future,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  in_memory_journal::InMemoryJournal, journal::Journal, journal_error::JournalError, persistent_repr::PersistentRepr,
};

fn noop_waker() -> Waker {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(|data| RawWaker::new(data, &VTABLE), |_| {}, |_| {}, |_| {});

  unsafe fn raw_waker() -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe { Waker::from_raw(raw_waker()) }
}

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);
  let mut future = Box::pin(future);
  match Future::poll(future.as_mut(), &mut cx) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was pending"),
  }
}

fn build_messages(persistence_id: &str, start: u64, count: u64) -> Vec<PersistentRepr> {
  (0..count)
    .map(|offset| {
      let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new((start + offset) as i32);
      PersistentRepr::new(persistence_id, start + offset, payload)
    })
    .collect()
}

#[test]
fn in_memory_journal_write_and_replay() {
  let mut journal = InMemoryJournal::new();
  let messages = build_messages("pid-1", 1, 3);

  let result = poll_ready(journal.write_messages(&messages));
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

  let result = poll_ready(journal.write_messages(&messages));
  assert_eq!(result, Err(JournalError::SequenceMismatch { expected: 1, actual: 2 }));
}

#[test]
fn in_memory_journal_replay_respects_max() {
  let mut journal = InMemoryJournal::new();
  let messages = build_messages("pid-1", 1, 3);
  poll_ready(journal.write_messages(&messages)).expect("write failed");

  let replayed = poll_ready(journal.replay_messages("pid-1", 1, 3, 1)).expect("replay failed");
  assert_eq!(replayed.len(), 1);
  assert_eq!(replayed[0].sequence_nr(), 1);
}

#[test]
fn in_memory_journal_delete_messages_to() {
  let mut journal = InMemoryJournal::new();
  let messages = build_messages("pid-1", 1, 3);
  poll_ready(journal.write_messages(&messages)).expect("write failed");

  poll_ready(journal.delete_messages_to("pid-1", 2)).expect("delete failed");

  let replayed = poll_ready(journal.replay_messages("pid-1", 1, 3, 10)).expect("replay failed");
  assert_eq!(replayed.len(), 1);
  assert_eq!(replayed[0].sequence_nr(), 3);
}

#[test]
fn in_memory_journal_highest_sequence_nr_defaults_to_zero() {
  let journal = InMemoryJournal::new();

  let highest = poll_ready(journal.highest_sequence_nr("missing")).expect("highest failed");
  assert_eq!(highest, 0);
}
