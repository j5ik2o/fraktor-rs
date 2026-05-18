//! Errors returned when querying cluster metrics.

/// Indicates why metrics cannot be returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetricsError {
  /// Metrics collection is disabled.
  Disabled,
}
