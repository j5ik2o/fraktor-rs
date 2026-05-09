use crate::RestartConfig;

#[derive(Debug, Clone)]
pub(crate) struct RestartBackoff {
  settings:              RestartConfig,
  restart_count:         usize,
  cooldown_ticks:        u32,
  pending:               bool,
  current_backoff_ticks: u32,
  last_schedule_tick:    u64,
  jitter_state:          u64,
}

impl RestartBackoff {
  pub(crate) fn new(min_backoff_ticks: u32, max_restarts: usize) -> Self {
    Self::from_settings(RestartConfig::new(min_backoff_ticks, min_backoff_ticks, max_restarts))
  }

  pub(crate) const fn from_settings(settings: RestartConfig) -> Self {
    let min_backoff_ticks = settings.min_backoff_ticks();
    let jitter_seed = settings.jitter_seed();
    Self {
      settings,
      restart_count: 0,
      cooldown_ticks: 0,
      pending: false,
      current_backoff_ticks: min_backoff_ticks,
      last_schedule_tick: 0,
      jitter_state: jitter_seed,
    }
  }

  pub(crate) const fn is_waiting(&self) -> bool {
    self.pending
  }

  pub(crate) const fn complete_on_max_restarts(&self) -> bool {
    self.settings.complete_on_max_restarts()
  }

  pub(crate) fn schedule(&mut self, now_tick: u64) -> bool {
    self.reset_backoff_if_window_elapsed(now_tick);
    if self.restart_count >= self.settings.max_restarts() {
      return false;
    }
    self.restart_count = self.restart_count.saturating_add(1);
    self.last_schedule_tick = now_tick;
    self.cooldown_ticks = self.next_cooldown_ticks();
    self.pending = true;
    true
  }

  pub(crate) fn tick(&mut self, now_tick: u64) -> bool {
    self.reset_backoff_if_window_elapsed(now_tick);
    if !self.pending {
      return false;
    }
    if self.cooldown_ticks > 0 {
      self.cooldown_ticks = self.cooldown_ticks.saturating_sub(1);
      return false;
    }
    self.pending = false;
    true
  }

  fn next_cooldown_ticks(&mut self) -> u32 {
    let min_ticks = self.settings.min_backoff_ticks();
    let max_ticks = self.settings.max_backoff_ticks();
    let base = self.current_backoff_ticks.max(min_ticks).min(max_ticks);
    let jitter_ticks = self.compute_jitter_ticks(base);
    self.current_backoff_ticks = base.saturating_mul(2).min(max_ticks).max(min_ticks);
    base.saturating_add(jitter_ticks).min(max_ticks)
  }

  fn reset_backoff_if_window_elapsed(&mut self, now_tick: u64) {
    let window = u64::from(self.settings.max_restarts_within_ticks());
    if window == 0 {
      return;
    }
    if now_tick.saturating_sub(self.last_schedule_tick) > window {
      self.current_backoff_ticks = self.settings.min_backoff_ticks();
    }
  }

  fn compute_jitter_ticks(&mut self, base_ticks: u32) -> u32 {
    let factor = u32::from(self.settings.random_factor_permille());
    if factor == 0 || base_ticks == 0 {
      return 0;
    }
    self.jitter_state = self.jitter_state.wrapping_mul(6364136223846793005).wrapping_add(1);
    let ratio_permille = (self.jitter_state >> 32) as u32 % 1001;
    base_ticks.saturating_mul(factor).saturating_mul(ratio_permille) / 1_000_000
  }
}
