use alloc::vec::Vec;

use super::DeterministicEvent;

pub(crate) struct DeterministicLog {
  entries:  Vec<DeterministicEvent>,
  capacity: usize,
}

impl DeterministicLog {
  pub(crate) fn with_capacity(capacity: usize) -> Self {
    Self { entries: Vec::with_capacity(capacity), capacity }
  }

  pub(crate) fn record(&mut self, event: DeterministicEvent) {
    if self.entries.len() < self.capacity {
      self.entries.push(event);
    }
  }

  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub(crate) fn entries(&self) -> &[DeterministicEvent] {
    &self.entries
  }
}

impl Clone for DeterministicLog {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), capacity: self.capacity }
  }
}
