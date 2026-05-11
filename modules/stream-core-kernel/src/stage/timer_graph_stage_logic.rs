use alloc::{collections::BTreeMap, vec::Vec};

#[cfg(test)]
#[path = "timer_graph_stage_logic_test.rs"]
mod tests;

/// Timer helper that tracks one-shot timers in graph stage logic.
pub struct TimerGraphStageLogic {
  current_tick: u64,
  schedules:    BTreeMap<u64, u64>,
}

impl TimerGraphStageLogic {
  /// Creates an empty timer state.
  #[must_use]
  pub const fn new() -> Self {
    Self { current_tick: 0, schedules: BTreeMap::new() }
  }

  /// Schedules a one-shot timer identified by `key`.
  pub fn schedule_once(&mut self, key: u64, delay_ticks: u64) {
    let due_tick = self.current_tick.saturating_add(delay_ticks);
    self.schedules.insert(key, due_tick);
  }

  /// Cancels a previously scheduled timer.
  #[must_use]
  pub fn cancel(&mut self, key: u64) -> bool {
    self.schedules.remove(&key).is_some()
  }

  /// Advances one tick and returns fired timer keys.
  #[must_use]
  pub fn advance(&mut self) -> Vec<u64> {
    self.current_tick = self.current_tick.saturating_add(1);
    let mut fired = Vec::new();
    for (key, due_tick) in &self.schedules {
      if *due_tick <= self.current_tick {
        fired.push(*key);
      }
    }
    for key in &fired {
      self.schedules.remove(key);
    }
    fired
  }

  /// Returns `true` if the timer key is scheduled.
  #[must_use]
  pub fn is_timer_active(&self, key: u64) -> bool {
    self.schedules.contains_key(&key)
  }

  /// Returns the current tick cursor.
  #[must_use]
  pub const fn current_tick(&self) -> u64 {
    self.current_tick
  }
}

impl Default for TimerGraphStageLogic {
  fn default() -> Self {
    Self::new()
  }
}
