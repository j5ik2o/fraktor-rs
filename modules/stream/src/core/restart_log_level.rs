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
