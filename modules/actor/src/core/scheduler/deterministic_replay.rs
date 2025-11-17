use super::DeterministicEvent;

/// Iterator over recorded deterministic events.
pub struct DeterministicReplay<'a> {
  events:   &'a [DeterministicEvent],
  position: usize,
}

impl<'a> DeterministicReplay<'a> {
  pub(crate) const fn new(events: &'a [DeterministicEvent]) -> Self {
    Self { events, position: 0 }
  }

  /// Returns the remaining events without advancing the iterator.
  #[must_use]
  pub const fn as_slice(&self) -> &'a [DeterministicEvent] {
    self.events
  }
}

impl<'a> Iterator for DeterministicReplay<'a> {
  type Item = DeterministicEvent;

  fn next(&mut self) -> Option<Self::Item> {
    if self.position >= self.events.len() {
      return None;
    }
    let event = self.events[self.position];
    self.position += 1;
    Some(event)
  }
}
