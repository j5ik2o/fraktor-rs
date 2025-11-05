use core::cmp::Ordering;

/// Entry stored inside the binary heap.
#[derive(Debug)]
pub(crate) struct PriorityEntry<T> {
  priority: i8,
  sequence: u64,
  item:     T,
}

impl<T> PriorityEntry<T> {
  pub(crate) const fn new(priority: i8, sequence: u64, item: T) -> Self {
    Self { priority, sequence, item }
  }

  pub(crate) const fn priority(&self) -> i8 {
    self.priority
  }

  pub(crate) const fn sequence(&self) -> u64 {
    self.sequence
  }

  pub(crate) const fn item(&self) -> &T {
    &self.item
  }

  pub(crate) fn into_item(self) -> T {
    self.item
  }
}

impl<T> PartialEq for PriorityEntry<T> {
  fn eq(&self, other: &Self) -> bool {
    self.priority == other.priority && self.sequence == other.sequence
  }
}

impl<T> Eq for PriorityEntry<T> {}

impl<T> PartialOrd for PriorityEntry<T> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<T> Ord for PriorityEntry<T> {
  fn cmp(&self, other: &Self) -> Ordering {
    match self.priority.cmp(&other.priority) {
      | Ordering::Equal => other.sequence.cmp(&self.sequence),
      | ord => ord,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

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
}
