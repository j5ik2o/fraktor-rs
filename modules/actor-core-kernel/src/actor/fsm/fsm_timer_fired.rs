//! Envelope used by classic FSM named timers.

#[cfg(test)]
#[path = "fsm_timer_fired_test.rs"]
mod tests;

use alloc::string::String;

use crate::actor::messaging::AnyMessage;

/// Internal wrapper sent by named FSM timers.
///
/// Handlers receive the wrapped payload, not this envelope. The type is
/// exported so message trait bounds propagate through public APIs, while
/// construction and accessors remain crate-internal.
#[derive(Clone)]
pub struct FsmTimerFired {
  name:       String,
  generation: u64,
  payload:    AnyMessage,
}

impl FsmTimerFired {
  pub(crate) const fn new(name: String, generation: u64, payload: AnyMessage) -> Self {
    Self { name, generation, payload }
  }

  pub(crate) fn name(&self) -> &str {
    &self.name
  }

  pub(crate) const fn generation(&self) -> u64 {
    self.generation
  }

  pub(crate) const fn payload(&self) -> &AnyMessage {
    &self.payload
  }
}
