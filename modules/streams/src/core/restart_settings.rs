#[cfg(test)]
mod tests;

/// Restart and backoff configuration for stream stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartSettings {
  min_backoff_ticks:         u32,
  max_backoff_ticks:         u32,
  random_factor_permille:    u16,
  max_restarts:              usize,
  max_restarts_within_ticks: u32,
  complete_on_max_restarts:  bool,
  jitter_seed:               u64,
}

impl RestartSettings {
  /// Creates restart settings with required fields.
  #[must_use]
  pub const fn new(min_backoff_ticks: u32, max_backoff_ticks: u32, max_restarts: usize) -> Self {
    let normalized_max_backoff =
      if max_backoff_ticks < min_backoff_ticks { min_backoff_ticks } else { max_backoff_ticks };
    Self {
      min_backoff_ticks,
      max_backoff_ticks: normalized_max_backoff,
      random_factor_permille: 0,
      max_restarts,
      max_restarts_within_ticks: u32::MAX,
      complete_on_max_restarts: true,
      jitter_seed: 0,
    }
  }

  /// Sets random factor as permille (`1000` means 100% jitter).
  #[must_use]
  pub const fn with_random_factor_permille(mut self, random_factor_permille: u16) -> Self {
    self.random_factor_permille = if random_factor_permille > 1000 { 1000 } else { random_factor_permille };
    self
  }

  /// Sets restart window ticks used for backoff reset.
  #[must_use]
  pub const fn with_max_restarts_within_ticks(mut self, max_restarts_within_ticks: u32) -> Self {
    self.max_restarts_within_ticks = max_restarts_within_ticks;
    self
  }

  /// Sets terminal action when restart limit is exhausted.
  #[must_use]
  pub const fn with_complete_on_max_restarts(mut self, complete_on_max_restarts: bool) -> Self {
    self.complete_on_max_restarts = complete_on_max_restarts;
    self
  }

  /// Sets deterministic jitter seed.
  #[must_use]
  pub const fn with_jitter_seed(mut self, jitter_seed: u64) -> Self {
    self.jitter_seed = jitter_seed;
    self
  }

  /// Returns minimum backoff ticks.
  #[must_use]
  pub const fn min_backoff_ticks(self) -> u32 {
    self.min_backoff_ticks
  }

  /// Returns maximum backoff ticks.
  #[must_use]
  pub const fn max_backoff_ticks(self) -> u32 {
    self.max_backoff_ticks
  }

  /// Returns random factor in permille.
  #[must_use]
  pub const fn random_factor_permille(self) -> u16 {
    self.random_factor_permille
  }

  /// Returns maximum restart attempts.
  #[must_use]
  pub const fn max_restarts(self) -> usize {
    self.max_restarts
  }

  /// Returns backoff reset window in ticks.
  #[must_use]
  pub const fn max_restarts_within_ticks(self) -> u32 {
    self.max_restarts_within_ticks
  }

  /// Returns terminal action for exhausted restart budget.
  #[must_use]
  pub const fn complete_on_max_restarts(self) -> bool {
    self.complete_on_max_restarts
  }

  /// Returns deterministic jitter seed.
  #[must_use]
  pub const fn jitter_seed(self) -> u64 {
    self.jitter_seed
  }
}
