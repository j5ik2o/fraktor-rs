//! Timer entries stored inside the wheel.

use super::TimerInstant;

/// Timer entry scheduled on the wheel.
#[derive(Clone, Debug)]
pub struct TimerEntry<P> {
  deadline: TimerInstant,
  payload:  P,
}

impl<P> TimerEntry<P> {
  /// Creates a one-shot entry with the specified payload.
  #[must_use]
  pub const fn oneshot(deadline: TimerInstant, payload: P) -> Self {
    Self { deadline, payload }
  }

  /// Returns the deadline instant.
  #[must_use]
  pub const fn deadline(&self) -> TimerInstant {
    self.deadline
  }

  /// Consumes the entry and returns its payload.
  #[must_use]
  pub fn into_payload(self) -> P {
    self.payload
  }
}
