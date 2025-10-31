use core::cmp::Ordering;

/// Entry stored inside the binary heap.
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
