//! Tick driver variant classification.

/// Classification of tick driver implementations.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickDriverKind {
  /// Automatic driver selected by runtime detection.
  Auto,
  /// Manual test driver.
  Manual,
  /// `std::thread`-based driver.
  Std,
  /// Tokio-based driver.
  Tokio,
  /// Embassy-based driver.
  Embassy,
}
