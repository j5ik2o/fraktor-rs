//! Non-thread-safe mutable message buffer for use inside actors.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

use crate::core::{actor::actor_ref::ActorRef, messaging::AnyMessage};

/// A non-thread-safe mutable message buffer that can be used to buffer messages inside actors.
///
/// This is a FIFO buffer backed by a [`VecDeque`]. Each entry stores a message alongside the
/// sender actor reference. Corresponds to Pekko's `org.apache.pekko.util.MessageBuffer`.
pub struct MessageBuffer {
  entries: VecDeque<(AnyMessage, ActorRef)>,
}

impl MessageBuffer {
  /// Creates an empty message buffer.
  #[must_use]
  pub const fn empty() -> Self {
    Self { entries: VecDeque::new() }
  }

  /// Returns `true` if the buffer contains no elements.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Returns the number of elements in the buffer.
  #[must_use]
  pub fn size(&self) -> usize {
    self.entries.len()
  }

  /// Appends a message and sender to the end of the buffer.
  pub fn append(&mut self, message: AnyMessage, ref_: ActorRef) {
    self.entries.push_back((message, ref_));
  }

  /// Returns a reference to the first element, or `None` if the buffer is empty.
  #[must_use]
  pub fn head(&self) -> Option<(&AnyMessage, &ActorRef)> {
    self.entries.front().map(|(m, r)| (m, r))
  }

  /// Removes and returns the first element from the buffer.
  #[must_use]
  pub fn pop_head(&mut self) -> Option<(AnyMessage, ActorRef)> {
    self.entries.pop_front()
  }

  /// Removes the first element from the buffer, discarding it.
  pub fn drop_head(&mut self) {
    self.entries.pop_front();
  }

  /// Iterates over all elements and applies the given closure to each `(message, ref)` pair.
  pub fn for_each<F>(&self, mut f: F)
  where
    F: FnMut(&AnyMessage, &ActorRef), {
    for (msg, ref_) in &self.entries {
      f(msg, ref_);
    }
  }

  /// Retains only the elements for which the predicate returns `true`.
  pub fn retain<F>(&mut self, mut predicate: F)
  where
    F: FnMut(&AnyMessage, &ActorRef) -> bool, {
    self.entries.retain(|(msg, ref_)| predicate(msg, ref_));
  }
}

impl Default for MessageBuffer {
  fn default() -> Self {
    Self::empty()
  }
}
