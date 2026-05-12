#[cfg(test)]
#[path = "timeout_kind_test.rs"]
mod tests;

use core::fmt::{Display, Formatter, Result as FmtResult};

/// Classification discriminator for stream timeout conditions.
///
/// Mirrors Apache Pekko's `pekko.stream.StreamTimeoutException` sealed
/// hierarchy:
///
/// - [`Self::Backpressure`] → `BackpressureTimeoutException`
/// - [`Self::Completion`] → `CompletionTimeoutException`
/// - [`Self::Idle`] → `StreamIdleTimeoutException`
/// - [`Self::Initial`] → `InitialTimeoutException`
///
/// The [`Display`] rendering emits the stable lower-case identifier
/// (`"backpressure"` / `"completion"` / `"idle"` / `"initial"`) that already
/// appears in existing timeout stage logics. This keeps the label format
/// suitable for interpolation into diagnostic messages (e.g. via
/// `StreamError::Timeout`) without embedding human-facing prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutKind {
  /// Downstream demand did not arrive within the configured window.
  Backpressure,
  /// Overall completion did not happen within the configured window.
  Completion,
  /// No element flowed through the stream within the configured window.
  Idle,
  /// The very first element did not arrive within the configured window.
  Initial,
}

impl Display for TimeoutKind {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    let label = match self {
      | Self::Backpressure => "backpressure",
      | Self::Completion => "completion",
      | Self::Idle => "idle",
      | Self::Initial => "initial",
    };
    f.write_str(label)
  }
}
