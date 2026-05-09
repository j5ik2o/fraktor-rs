#[cfg(test)]
mod tests;

use crate::attributes::{LogLevel, SourceLocation};

/// Materializer-level logging backend contract.
///
/// Mirrors Apache Pekko's `pekko.stream.MaterializerLoggingProvider`.
/// A materializer exposes this trait so stage logic can emit log messages
/// without depending on a concrete logger. Both methods take `&self` so the
/// provider can be shared through a trait object and queried from a CQS
/// query-side context.
///
/// `is_enabled` is intended for hot-path bypass checks and must remain
/// side-effect free. `log` receives an optional [`SourceLocation`] so
/// adapters can surface where in the user's code the event originated.
pub trait MaterializerLoggingProvider {
  /// Returns whether messages at `level` would be emitted by this provider.
  ///
  /// Callers may use this to skip expensive message formatting. `LogLevel::Off`
  /// is expected to always return `false`.
  fn is_enabled(&self, level: LogLevel) -> bool;

  /// Emits `message` at `level` with an optional source location.
  ///
  /// Implementations must not panic for any supported level or message. The
  /// `source_location` is borrowed for the duration of the call and must
  /// therefore be cloned if the implementation needs to retain it.
  fn log(&self, level: LogLevel, message: &str, source_location: Option<&SourceLocation>);
}
