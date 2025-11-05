use alloc::vec;
use core::cmp::Ordering;

use super::PriorityEntry;

#[test]
fn priority_entry_new() {
  let entry = PriorityEntry::new(5, 100, "test");
  assert_eq!(entry.priority(), 5);
  assert_eq!(entry.sequence(), 100);
  assert_eq!(entry.item(), &"test");
}

#[test]
fn priority_entry_into_item() {
  let entry = PriorityEntry::new(3, 50, vec![1, 2, 3]);
  let item = entry.into_item();
  assert_eq!(item, vec![1, 2, 3]);
}

#[test]
fn priority_entry_partial_eq() {
  let entry1 = PriorityEntry::new(5, 10, 100);
  let entry2 = PriorityEntry::new(5, 10, 200);
  let entry3 = PriorityEntry::new(5, 11, 100);
  assert_eq!(entry1, entry2);
  assert_ne!(entry1, entry3);
}

#[test]
fn priority_entry_cmp_by_priority() {
  let entry1 = PriorityEntry::new(5, 10, ());
  let entry2 = PriorityEntry::new(3, 10, ());
  assert!(entry1 > entry2);
  assert!(entry2 < entry1);
}

#[test]
fn priority_entry_cmp_by_sequence_when_priority_equal() {
  let entry1 = PriorityEntry::new(5, 10, ());
  let entry2 = PriorityEntry::new(5, 20, ());
  // 同じpriorityの場合、sequenceが大きいほうが小さい（FIFO）
  assert!(entry1 > entry2);
  assert!(entry2 < entry1);
}

#[test]
fn priority_entry_partial_ord() {
  let entry1 = PriorityEntry::new(5, 10, ());
  let entry2 = PriorityEntry::new(3, 10, ());
  assert_eq!(entry1.partial_cmp(&entry2), Some(Ordering::Greater));
}

#[test]
fn priority_entry_ord_consistent_with_eq() {
  let entry1 = PriorityEntry::new(5, 10, 1);
  let entry2 = PriorityEntry::new(5, 10, 2);
  assert_eq!(entry1.cmp(&entry2), Ordering::Equal);
  assert_eq!(entry1, entry2);
}
