//! Timer entries stored inside the wheel.

use core::fmt::Debug;

use super::{TimerEntryMode, TimerInstant};

/// Timer entry scheduled on the wheel.
#[derive(Clone, Debug)]
pub struct TimerEntry<P> {
  deadline: TimerInstant,
  mode:     TimerEntryMode,
  payload:  P,
}

impl<P> TimerEntry<P> {
  /// Creates a one-shot entry with the specified payload.
  #[must_use]
  pub fn oneshot(deadline: TimerInstant, payload: P) -> Self {
    Self { deadline, mode: TimerEntryMode::OneShot, payload }
  }

  /// Returns the deadline instant.
  #[must_use]
  pub fn deadline(&self) -> TimerInstant {
    self.deadline
  }

  /// Returns the execution mode.
  #[must_use]
  pub fn mode(&self) -> TimerEntryMode {
    self.mode
  }

  /// Consumes the entry and returns its payload.
  #[must_use]
  pub fn into_payload(self) -> P {
    self.payload
  }
}
