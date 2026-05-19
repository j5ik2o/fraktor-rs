//! Internal state for classic FSM named timers.

use alloc::string::String;

pub(crate) struct FsmNamedTimer {
  generation:   u64,
  is_repeating: bool,
  timer_key:    String,
}

impl FsmNamedTimer {
  pub(crate) const fn new(generation: u64, is_repeating: bool, timer_key: String) -> Self {
    Self { generation, is_repeating, timer_key }
  }

  pub(crate) const fn generation(&self) -> u64 {
    self.generation
  }

  pub(crate) const fn is_repeating(&self) -> bool {
    self.is_repeating
  }

  pub(crate) fn timer_key(&self) -> &str {
    &self.timer_key
  }
}
