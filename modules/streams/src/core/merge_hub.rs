use alloc::collections::VecDeque;

#[cfg(test)]
mod tests;

/// Minimal merge hub that merges offered elements into a single queue.
pub struct MergeHub<T> {
  queue: VecDeque<T>,
}

impl<T> MergeHub<T> {
  /// Creates an empty merge hub.
  #[must_use]
  pub const fn new() -> Self {
    Self { queue: VecDeque::new() }
  }

  /// Offers an element into the hub.
  pub fn offer(&mut self, value: T) {
    self.queue.push_back(value);
  }

  /// Polls the next merged element from the hub.
  #[must_use]
  pub fn poll(&mut self) -> Option<T> {
    self.queue.pop_front()
  }

  /// Returns the number of queued elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.queue.len()
  }

  /// Returns true when the hub queue is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.queue.is_empty()
  }
}

impl<T> Default for MergeHub<T> {
  fn default() -> Self {
    Self::new()
  }
}
