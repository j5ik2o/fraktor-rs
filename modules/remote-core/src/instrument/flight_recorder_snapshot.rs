//! Immutable snapshot returned by the flight recorder.

use alloc::vec::Vec;

use crate::instrument::flight_recorder_event::FlightRecorderEvent;

/// Immutable snapshot of the flight recorder event buffer.
///
/// Produced by [`crate::instrument::RemotingFlightRecorder::snapshot`]. The
/// caller observes a `&[FlightRecorderEvent]` via [`Self::events`] and cannot
/// mutate the snapshot after construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotingFlightRecorderSnapshot {
  events: Vec<FlightRecorderEvent>,
}

impl RemotingFlightRecorderSnapshot {
  /// Creates a new snapshot wrapping the given events (oldest first).
  #[must_use]
  pub const fn new(events: Vec<FlightRecorderEvent>) -> Self {
    Self { events }
  }

  /// Returns the recorded events in insertion order (oldest first).
  #[must_use]
  pub fn events(&self) -> &[FlightRecorderEvent] {
    &self.events
  }

  /// Returns the number of events captured in the snapshot.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.events.len()
  }

  /// Returns `true` when the snapshot contains no events.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.events.is_empty()
  }
}
