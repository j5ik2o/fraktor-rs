//! Log severity levels used by the runtime.

/// Severity levels recognised by the event stream logger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
  /// Detailed trace information.
  Trace,
  /// Debug information useful during development.
  Debug,
  /// General informational messages.
  Info,
  /// Warnings indicating potential issues.
  Warn,
  /// Errors signalling failures that require attention.
  Error,
}
