//! Auto-detection profile classification for tick drivers.

/// Classification of auto-detected driver profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoProfileKind {
  /// Tokio runtime detected.
  Tokio,
  /// Embassy runtime detected.
  Embassy,
  /// Custom runtime.
  Custom,
}
