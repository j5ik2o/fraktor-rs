use alloc::{collections::BTreeSet, format, string::String};

use crate::delivery::{DurableProducerQueueState, MessageSent, NO_QUALIFIER, SeqNr};

fn make_sent(seq_nr: SeqNr, qualifier: &str) -> MessageSent<u32> {
  MessageSent::new(seq_nr, seq_nr as u32, false, String::from(qualifier), 0)
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn empty_creates_initial_state() {
  // Given/When
  let state = DurableProducerQueueState::<u32>::empty();

  // Then: initial values match Pekko's State.empty
  assert_eq!(state.current_seq_nr(), 1);
  assert_eq!(state.highest_confirmed_seq_nr(), 0);
  assert!(state.confirmed_seq_nr().is_empty());
  assert!(state.unconfirmed().is_empty());
}

// ---------------------------------------------------------------------------
// add_message_sent
// ---------------------------------------------------------------------------

#[test]
fn add_message_sent_appends_and_advances_seq_nr() {
  // Given
  let state = DurableProducerQueueState::<u32>::empty();
  let sent = make_sent(1, "");

  // When
  let state = state.add_message_sent(sent);

  // Then: current_seq_nr = sent.seq_nr + 1, unconfirmed grows
  assert_eq!(state.current_seq_nr(), 2);
  assert_eq!(state.unconfirmed().len(), 1);
  assert_eq!(state.unconfirmed()[0].seq_nr(), 1);
}

#[test]
fn add_message_sent_twice_keeps_order() {
  // Given
  let state = DurableProducerQueueState::<u32>::empty();
  let sent1 = make_sent(1, "");
  let sent2 = make_sent(2, "");

  // When
  let state = state.add_message_sent(sent1).add_message_sent(sent2);

  // Then
  assert_eq!(state.current_seq_nr(), 3);
  assert_eq!(state.unconfirmed().len(), 2);
  assert_eq!(state.unconfirmed()[0].seq_nr(), 1);
  assert_eq!(state.unconfirmed()[1].seq_nr(), 2);
}

// ---------------------------------------------------------------------------
// confirmed
// ---------------------------------------------------------------------------

#[test]
fn confirmed_removes_matching_unconfirmed() {
  // Given: state with 3 unconfirmed messages (same qualifier)
  let state = DurableProducerQueueState::<u32>::empty()
    .add_message_sent(make_sent(1, ""))
    .add_message_sent(make_sent(2, ""))
    .add_message_sent(make_sent(3, ""));

  // When: confirm up to seq_nr 2
  let state = state.confirmed(2, NO_QUALIFIER.clone(), 1000);

  // Then: messages 1 and 2 are removed, 3 remains
  assert_eq!(state.unconfirmed().len(), 1);
  assert_eq!(state.unconfirmed()[0].seq_nr(), 3);
  assert_eq!(state.highest_confirmed_seq_nr(), 2);
}

#[test]
fn confirmed_updates_highest_confirmed_seq_nr() {
  // Given
  let state = DurableProducerQueueState::<u32>::empty().add_message_sent(make_sent(1, ""));

  // When
  let state = state.confirmed(1, NO_QUALIFIER.clone(), 500);

  // Then
  assert_eq!(state.highest_confirmed_seq_nr(), 1);
}

#[test]
fn confirmed_with_qualifier_only_removes_matching_qualifier() {
  // Given: messages with different qualifiers
  let state = DurableProducerQueueState::<u32>::empty()
    .add_message_sent(make_sent(1, "topic-A"))
    .add_message_sent(make_sent(2, "topic-B"))
    .add_message_sent(make_sent(3, "topic-A"));

  // When: confirm topic-A up to seq_nr 3
  let state = state.confirmed(3, String::from("topic-A"), 1000);

  // Then: only topic-A messages (1, 3) are removed; topic-B (2) remains
  assert_eq!(state.unconfirmed().len(), 1);
  assert_eq!(state.unconfirmed()[0].seq_nr(), 2);
  assert_eq!(state.unconfirmed()[0].confirmation_qualifier(), "topic-B");
}

#[test]
fn confirmed_stores_qualifier_entry_in_confirmed_seq_nr_map() {
  // Given
  let state = DurableProducerQueueState::<u32>::empty().add_message_sent(make_sent(1, "q1"));

  // When
  let state = state.confirmed(1, String::from("q1"), 999);

  // Then: confirmed_seq_nr map contains the entry
  let entry = state.confirmed_seq_nr().get("q1");
  assert!(entry.is_some());
  let (seq_nr, ts) = entry.unwrap();
  assert_eq!(*seq_nr, 1);
  assert_eq!(*ts, 999);
}

#[test]
fn confirmed_idempotent_duplicate_does_not_increase_highest() {
  // Given: already confirmed up to 5
  let state = DurableProducerQueueState::<u32>::empty().add_message_sent(make_sent(1, "")).confirmed(
    5,
    NO_QUALIFIER.clone(),
    100,
  );

  // When: confirm with lower seq_nr (idempotent replay)
  let state = state.confirmed(3, NO_QUALIFIER.clone(), 200);

  // Then: highest_confirmed_seq_nr should remain at 5 (max)
  assert_eq!(state.highest_confirmed_seq_nr(), 5);
}

// ---------------------------------------------------------------------------
// cleanup
// ---------------------------------------------------------------------------

#[test]
fn cleanup_removes_qualifier_entries() {
  // Given: state with two confirmed qualifiers
  let state = DurableProducerQueueState::<u32>::empty().confirmed(1, String::from("q1"), 100).confirmed(
    2,
    String::from("q2"),
    200,
  );
  assert_eq!(state.confirmed_seq_nr().len(), 2);

  // When: cleanup q1
  let qualifiers = {
    let mut set = BTreeSet::new();
    set.insert(String::from("q1"));
    set
  };
  let state = state.cleanup(&qualifiers);

  // Then: only q2 remains
  assert_eq!(state.confirmed_seq_nr().len(), 1);
  assert!(state.confirmed_seq_nr().contains_key("q2"));
}

#[test]
fn cleanup_with_nonexistent_qualifier_is_noop() {
  // Given: state with one confirmed qualifier
  let state = DurableProducerQueueState::<u32>::empty().confirmed(1, String::from("q1"), 100);

  // When: cleanup a qualifier that does not exist
  let qualifiers = {
    let mut set = BTreeSet::new();
    set.insert(String::from("nonexistent"));
    set
  };
  let state = state.cleanup(&qualifiers);

  // Then: q1 still exists
  assert_eq!(state.confirmed_seq_nr().len(), 1);
  assert!(state.confirmed_seq_nr().contains_key("q1"));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn confirmed_on_empty_state_is_safe() {
  // Given: empty state with no unconfirmed messages
  let state = DurableProducerQueueState::<u32>::empty();

  // When: confirming a seq_nr (no messages to remove)
  let state = state.confirmed(1, NO_QUALIFIER.clone(), 100);

  // Then: no panic, state is updated
  assert_eq!(state.highest_confirmed_seq_nr(), 1);
  assert!(state.unconfirmed().is_empty());
}

#[test]
fn debug_format_is_non_empty() {
  // Given
  let state = DurableProducerQueueState::<u32>::empty();

  // When
  let debug_str = format!("{:?}", state);

  // Then
  assert!(!debug_str.is_empty());
}
