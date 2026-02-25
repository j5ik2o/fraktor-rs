//! Replay adapter output representation.

#[cfg(test)]
mod tests;

use alloc::{vec, vec::Vec};
use core::{
  any::Any,
  fmt::{Debug, Formatter},
};

use fraktor_utils_rs::core::sync::ArcShared;

/// Sequence of zero, one, or many replayed events.
pub enum EventSeq {
  /// No events should be delivered.
  Empty,
  /// A single event should be delivered.
  Single(ArcShared<dyn Any + Send + Sync>),
  /// Multiple events should be delivered in order.
  Multiple(Vec<ArcShared<dyn Any + Send + Sync>>),
}

impl EventSeq {
  /// Creates an empty event sequence.
  #[must_use]
  pub const fn empty() -> Self {
    Self::Empty
  }

  /// Creates an event sequence with one event.
  #[must_use]
  pub fn single(event: ArcShared<dyn Any + Send + Sync>) -> Self {
    Self::Single(event)
  }

  /// Creates an event sequence from many events.
  #[must_use]
  pub fn multiple(events: Vec<ArcShared<dyn Any + Send + Sync>>) -> Self {
    match events.len() {
      | 0 => Self::Empty,
      | 1 => {
        let mut iter = events.into_iter();
        if let Some(event) = iter.next() { Self::Single(event) } else { Self::Empty }
      },
      | _ => Self::Multiple(events),
    }
  }

  /// Returns the number of contained events.
  #[must_use]
  pub const fn len(&self) -> usize {
    match self {
      | EventSeq::Empty => 0,
      | EventSeq::Single(_) => 1,
      | EventSeq::Multiple(events) => events.len(),
    }
  }

  /// Returns whether the sequence contains no events.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    matches!(self, Self::Empty)
  }

  /// Consumes the sequence and returns events as a vector.
  #[must_use]
  pub fn into_events(self) -> Vec<ArcShared<dyn Any + Send + Sync>> {
    match self {
      | EventSeq::Empty => Vec::new(),
      | EventSeq::Single(event) => vec![event],
      | EventSeq::Multiple(events) => events,
    }
  }
}

impl Default for EventSeq {
  fn default() -> Self {
    Self::empty()
  }
}

impl Debug for EventSeq {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    match self {
      | EventSeq::Empty => f.debug_tuple("EventSeq::Empty").finish(),
      | EventSeq::Single(_) => f.debug_tuple("EventSeq::Single").field(&"<any>").finish(),
      | EventSeq::Multiple(events) => f.debug_struct("EventSeq::Multiple").field("len", &events.len()).finish(),
    }
  }
}
