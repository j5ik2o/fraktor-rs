//! Key-indexed collection of message buffers for use inside actors.

#[cfg(test)]
#[path = "message_buffer_map_test.rs"]
mod tests;

use core::hash::Hash;

use ahash::RandomState;
use hashbrown::HashMap;

use super::MessageBuffer;
use crate::actor::{actor_ref::ActorRef, messaging::AnyMessage};

/// A non-thread-safe mutable message buffer map that can be used to buffer messages inside actors.
///
/// Each key of type `I` maps to its own [`MessageBuffer`]. Corresponds to Pekko's
/// `org.apache.pekko.util.MessageBufferMap`.
pub struct MessageBufferMap<I> {
  buffers: HashMap<I, MessageBuffer, RandomState>,
}

impl<I: Eq + Hash> MessageBufferMap<I> {
  /// Creates an empty message buffer map.
  #[must_use]
  pub fn empty() -> Self {
    Self { buffers: HashMap::with_hasher(RandomState::new()) }
  }

  /// Returns `true` if the map contains no ids.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.buffers.is_empty()
  }

  /// Returns the number of ids in the map.
  #[must_use]
  pub fn size(&self) -> usize {
    self.buffers.len()
  }

  /// Returns the total number of buffered messages across all ids.
  #[must_use]
  pub fn total_size(&self) -> usize {
    self.buffers.values().map(MessageBuffer::size).sum()
  }

  /// Ensures the given id exists in the map (creates an empty buffer if absent).
  pub fn add(&mut self, id: I) {
    self.buffers.entry(id).or_insert_with(MessageBuffer::empty);
  }

  /// Appends a message and sender to the buffer for the given id.
  pub fn append(&mut self, id: I, message: AnyMessage, ref_: ActorRef) {
    self.buffers.entry(id).or_insert_with(MessageBuffer::empty).append(message, ref_);
  }

  /// Removes the buffer for the given id.
  pub fn remove(&mut self, id: &I) {
    self.buffers.remove(id);
  }

  /// Returns `true` if the map contains a buffer for the given id.
  #[must_use]
  pub fn contains(&self, id: &I) -> bool {
    self.buffers.contains_key(id)
  }

  /// Returns a reference to the buffer for the given id, or `None` if absent.
  #[must_use]
  pub fn get(&self, id: &I) -> Option<&MessageBuffer> {
    self.buffers.get(id)
  }

  /// Returns a mutable reference to the buffer for the given id, or `None` if absent.
  #[must_use]
  pub fn get_mut(&mut self, id: &I) -> Option<&mut MessageBuffer> {
    self.buffers.get_mut(id)
  }

  /// Iterates over all `(id, buffer)` pairs and applies the given closure.
  pub fn for_each<F>(&self, mut f: F)
  where
    F: FnMut(&I, &MessageBuffer), {
    for (id, buf) in &self.buffers {
      f(id, buf);
    }
  }
}

impl<I: Eq + Hash> Default for MessageBufferMap<I> {
  fn default() -> Self {
    Self::empty()
  }
}
