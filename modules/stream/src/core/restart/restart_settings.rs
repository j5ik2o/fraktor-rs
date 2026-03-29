use core::fmt;

use fraktor_utils_rs::core::sync::ArcShared;

use super::restart_log_settings::RestartLogSettings;
use crate::core::stream_error::StreamError;

#[cfg(test)]
mod tests;

type RestartPredicate = dyn Fn(&StreamError) -> bool + Send + Sync;

/// Restart and backoff configuration for stream stages.
#[derive(Clone)]
pub struct RestartSettings {
  min_backoff_ticks:         u32,
  max_backoff_ticks:         u32,
  random_factor_permille:    u16,
  max_restarts:              usize,
  max_restarts_within_ticks: u32,
  complete_on_max_restarts:  bool,
  jitter_seed:               u64,
  restart_on:                Option<ArcShared<RestartPredicate>>,
  log_settings:              RestartLogSettings,
}

// PartialEq/Eq は restart_on (クロージャ) を含むため正確な等値比較ができず削除。
// should_restart() の結果が異なる RestartSettings 同士が == になる問題を防ぐ。

impl fmt::Debug for RestartSettings {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RestartSettings")
      .field("min_backoff_ticks", &self.min_backoff_ticks)
      .field("max_backoff_ticks", &self.max_backoff_ticks)
      .field("random_factor_permille", &self.random_factor_permille)
      .field("max_restarts", &self.max_restarts)
      .field("max_restarts_within_ticks", &self.max_restarts_within_ticks)
      .field("complete_on_max_restarts", &self.complete_on_max_restarts)
      .field("jitter_seed", &self.jitter_seed)
      .field("restart_on", &self.restart_on.as_ref().map(|_| ".."))
      .field("log_settings", &self.log_settings)
      .finish()
  }
}

impl RestartSettings {
  /// Creates restart settings with required fields.
  #[must_use]
  pub fn new(min_backoff_ticks: u32, max_backoff_ticks: u32, max_restarts: usize) -> Self {
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
      restart_on: None,
      log_settings: RestartLogSettings::default(),
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

  /// Sets a predicate to determine whether a given error should trigger a restart.
  ///
  /// When `None` (default), all errors trigger a restart.
  #[must_use]
  pub fn with_restart_on<F>(mut self, predicate: F) -> Self
  where
    F: Fn(&StreamError) -> bool + Send + Sync + 'static, {
    self.restart_on = Some(ArcShared::new(predicate));
    self
  }

  /// Sets log settings for restart event diagnostics.
  #[must_use]
  pub const fn with_log_settings(mut self, log_settings: RestartLogSettings) -> Self {
    self.log_settings = log_settings;
    self
  }

  /// Returns `true` if the given error should trigger a restart.
  ///
  /// When no predicate is configured (default), all errors trigger a restart.
  #[must_use]
  pub fn should_restart(&self, error: &StreamError) -> bool {
    match &self.restart_on {
      | Some(predicate) => predicate(error),
      | None => true,
    }
  }

  /// Returns minimum backoff ticks.
  #[must_use]
  pub const fn min_backoff_ticks(&self) -> u32 {
    self.min_backoff_ticks
  }

  /// Returns maximum backoff ticks.
  #[must_use]
  pub const fn max_backoff_ticks(&self) -> u32 {
    self.max_backoff_ticks
  }

  /// Returns random factor in permille.
  #[must_use]
  pub const fn random_factor_permille(&self) -> u16 {
    self.random_factor_permille
  }

  /// Returns maximum restart attempts.
  #[must_use]
  pub const fn max_restarts(&self) -> usize {
    self.max_restarts
  }

  /// Returns backoff reset window in ticks.
  #[must_use]
  pub const fn max_restarts_within_ticks(&self) -> u32 {
    self.max_restarts_within_ticks
  }

  /// Returns terminal action for exhausted restart budget.
  #[must_use]
  pub const fn complete_on_max_restarts(&self) -> bool {
    self.complete_on_max_restarts
  }

  /// Returns deterministic jitter seed.
  #[must_use]
  pub const fn jitter_seed(&self) -> u64 {
    self.jitter_seed
  }

  /// Returns log settings for restart event diagnostics.
  #[must_use]
  pub const fn log_settings(&self) -> &RestartLogSettings {
    &self.log_settings
  }
}
