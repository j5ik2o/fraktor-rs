use alloc::string::ToString;
use core::num::NonZeroUsize;

use super::CompressionTable;
use crate::wire::{CompressionTableEntry, CompressionTableKind, WireError};

fn max(value: usize) -> Option<NonZeroUsize> {
  NonZeroUsize::new(value)
}

#[test]
fn observe_updates_hit_count_without_duplicate_entry_ids() {
  let mut table = CompressionTable::new(max(4));

  table.observe("/user/a");
  table.observe("/user/a");

  assert_eq!(table.len(), 1);
  assert_eq!(table.hit_count("/user/a"), Some(2));
  assert_eq!(table.entry_id("/user/a"), Some(1));
}

#[test]
fn observe_stops_adding_entries_at_configured_max() {
  let mut table = CompressionTable::new(max(1));

  table.observe("/user/a");
  table.observe("/user/b");
  table.observe("/user/a");

  assert_eq!(table.len(), 1);
  assert_eq!(table.entry_id("/user/b"), None);
  assert_eq!(table.hit_count("/user/a"), Some(2));
}

#[test]
fn max_accessor_returns_configured_bound() {
  let table = CompressionTable::new(max(2));

  assert_eq!(table.max(), max(2));
}

#[test]
fn empty_table_does_not_create_advertisement() {
  let mut table = CompressionTable::new(max(2));

  assert!(table.create_advertisement(CompressionTableKind::ActorRef).is_none());
}

#[test]
fn disabled_table_does_not_track_hits_or_advertise() {
  let mut table = CompressionTable::new(None);

  table.observe("/user/a");

  assert!(table.is_empty());
  assert!(table.create_advertisement(CompressionTableKind::ActorRef).is_none());
  assert_eq!(table.encode("/user/a").as_literal(), Some("/user/a"));
  assert_eq!(table.resolve(1), None);
}

#[test]
fn advertisement_is_bounded_and_deterministic() {
  let mut table = CompressionTable::new(max(3));
  table.observe("/user/a");
  table.observe("/user/b");
  table.observe("/user/b");
  table.observe("/user/c");
  table.observe("/user/c");

  let advertisement = table.create_advertisement(CompressionTableKind::ActorRef).unwrap();

  assert_eq!(advertisement.generation(), 1);
  assert_eq!(advertisement.entries().len(), 3);
  assert_eq!(advertisement.entries()[0].literal(), "/user/b");
  assert_eq!(advertisement.entries()[1].literal(), "/user/c");
  assert_eq!(advertisement.entries()[2].literal(), "/user/a");
}

#[test]
fn acked_entries_encode_as_table_refs() {
  let mut table = CompressionTable::new(max(4));
  table.observe("/user/a");
  let advertisement = table.create_advertisement(CompressionTableKind::ActorRef).unwrap();
  let entry_id = advertisement.entries()[0].id();

  assert_eq!(table.encode("/user/a").as_literal(), Some("/user/a"));
  assert!(table.acknowledge(advertisement.generation()));

  assert_eq!(table.encode("/user/a").as_table_ref(), Some(entry_id));
}

#[test]
fn stale_ack_is_ignored() {
  let mut table = CompressionTable::new(max(4));
  table.observe("/user/a");
  let generation_1 = table.create_advertisement(CompressionTableKind::ActorRef).unwrap().generation();
  let generation_2 = table.create_advertisement(CompressionTableKind::ActorRef).unwrap().generation();

  assert!(!table.acknowledge(generation_1));
  assert_eq!(table.latest_pending_generation(), Some(generation_2));
  assert_eq!(table.encode("/user/a").as_literal(), Some("/user/a"));
}

#[test]
fn inbound_advertisement_resolves_entry_ids() {
  let mut table = CompressionTable::new(max(4));
  let entries = [CompressionTableEntry::new(9, "/user/a".to_string())];

  assert_eq!(table.apply_advertisement(7, &entries), Ok(true));

  assert_eq!(table.resolve(9), Some("/user/a"));
  assert_eq!(table.resolve(10), None);
}

#[test]
fn inbound_advertisement_rejects_entries_over_configured_max() {
  let mut table = CompressionTable::new(max(1));
  let entries =
    [CompressionTableEntry::new(9, "/user/a".to_string()), CompressionTableEntry::new(10, "/user/b".to_string())];

  let err = table.apply_advertisement(7, &entries).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
  assert!(table.is_empty());
}

#[test]
fn duplicate_inbound_entry_ids_are_rejected() {
  let mut table = CompressionTable::new(max(4));
  let entries =
    [CompressionTableEntry::new(9, "/user/a".to_string()), CompressionTableEntry::new(9, "/user/b".to_string())];

  let err = table.apply_advertisement(7, &entries).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}
