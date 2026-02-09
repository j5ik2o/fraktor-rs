use alloc::{collections::VecDeque, vec::Vec};

#[cfg(test)]
mod tests;

/// Minimal broadcast hub that fans out each element to every subscriber.
pub struct BroadcastHub<T> {
  subscribers: Vec<VecDeque<T>>,
}

impl<T> BroadcastHub<T>
where
  T: Clone,
{
  /// Creates an empty broadcast hub.
  #[must_use]
  pub const fn new() -> Self {
    Self { subscribers: Vec::new() }
  }

  /// Adds a subscriber and returns its identifier.
  #[must_use]
  pub fn subscribe(&mut self) -> usize {
    self.subscribers.push(VecDeque::new());
    self.subscribers.len().saturating_sub(1)
  }

  /// Publishes an element to all subscribers.
  pub fn publish(&mut self, value: T) {
    for queue in &mut self.subscribers {
      queue.push_back(value.clone());
    }
  }

  /// Polls the next element for the specified subscriber.
  #[must_use]
  pub fn poll(&mut self, subscriber_id: usize) -> Option<T> {
    self.subscribers.get_mut(subscriber_id).and_then(VecDeque::pop_front)
  }
}

impl<T> Default for BroadcastHub<T>
where
  T: Clone,
{
  fn default() -> Self {
    Self::new()
  }
}
