//! Typed replay adapter output representation.

#[cfg(test)]
#[path = "event_seq_test.rs"]
mod tests;

use alloc::{vec, vec::Vec};
use core::fmt::{Debug, Formatter, Result as FmtResult};

/// Sequence of zero, one, or many typed replay events.
#[derive(Clone, PartialEq, Eq)]
pub enum EventSeq<E> {
  /// No events should be delivered.
  Empty,
  /// A single event should be delivered.
  Single(E),
  /// Multiple events should be delivered in order.
  Multiple(Vec<E>),
}

impl<E> EventSeq<E> {
  /// Creates an empty event sequence.
  #[must_use]
  pub const fn empty() -> Self {
    Self::Empty
  }

  /// Creates an event sequence with one event.
  #[must_use]
  pub const fn single(event: E) -> Self {
    Self::Single(event)
  }

  /// Creates an event sequence from many events.
  #[must_use]
  pub fn multiple(events: Vec<E>) -> Self {
    match events.len() {
      | 0 => Self::Empty,
      | 1 => events.into_iter().next().map_or(Self::Empty, Self::Single),
      | _ => Self::Multiple(events),
    }
  }

  /// Returns the number of contained events.
  #[must_use]
  pub const fn len(&self) -> usize {
    match self {
      | Self::Empty => 0,
      | Self::Single(_) => 1,
      | Self::Multiple(events) => events.len(),
    }
  }

  /// Returns whether the sequence contains no events.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    matches!(self, Self::Empty)
  }

  /// Consumes the sequence and returns events as a vector.
  #[must_use]
  pub fn into_events(self) -> Vec<E> {
    match self {
      | Self::Empty => Vec::new(),
      | Self::Single(event) => vec![event],
      | Self::Multiple(events) => events,
    }
  }
}

impl<E> Default for EventSeq<E> {
  fn default() -> Self {
    Self::empty()
  }
}

impl<E> Debug for EventSeq<E> {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::Empty => formatter.debug_tuple("EventSeq::Empty").finish(),
      | Self::Single(_) => formatter.debug_tuple("EventSeq::Single").field(&"<event>").finish(),
      | Self::Multiple(events) => formatter.debug_struct("EventSeq::Multiple").field("len", &events.len()).finish(),
    }
  }
}
