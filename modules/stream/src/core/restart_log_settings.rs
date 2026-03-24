/// Log level for restart event diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartLogLevel {
  /// Verbose diagnostic information.
  Debug,
  /// Informational restart events.
  Info,
  /// Potentially harmful restart events (default for normal restarts).
  Warning,
  /// Restart events indicating failures (default for critical restarts).
  Error,
}

/// Log settings for restart event diagnostics.
///
/// Controls how restart events are logged at different severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartLogSettings {
  log_level:                RestartLogLevel,
  critical_log_level:       RestartLogLevel,
  critical_log_level_after: usize,
}

impl RestartLogSettings {
  /// Creates log settings with explicit values.
  #[must_use]
  pub const fn new(
    log_level: RestartLogLevel,
    critical_log_level: RestartLogLevel,
    critical_log_level_after: usize,
  ) -> Self {
    Self { log_level, critical_log_level, critical_log_level_after }
  }

  /// Returns the log level for normal restart events.
  #[must_use]
  pub const fn log_level(&self) -> RestartLogLevel {
    self.log_level
  }

  /// Returns the log level for critical restart events.
  #[must_use]
  pub const fn critical_log_level(&self) -> RestartLogLevel {
    self.critical_log_level
  }

  /// Returns the restart count threshold after which critical log level is used.
  #[must_use]
  pub const fn critical_log_level_after(&self) -> usize {
    self.critical_log_level_after
  }
}

impl Default for RestartLogSettings {
  fn default() -> Self {
    Self {
      log_level:                RestartLogLevel::Warning,
      critical_log_level:       RestartLogLevel::Error,
      critical_log_level_after: usize::MAX,
    }
  }
}
