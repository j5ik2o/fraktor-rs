//! Configuration for Tokio TCP transport.

/// Configuration for Tokio TCP transport.
///
/// This is a placeholder implementation. Future versions will include:
/// - TCP buffer sizes
/// - Connection timeout
/// - Keep-alive settings
/// - Reconnection policy
/// - TLS configuration
#[derive(Debug, Clone)]
pub struct TokioTransportConfig {
  // Placeholder for future configuration fields
  _reserved: (),
}

impl TokioTransportConfig {
  /// Creates a new Tokio transport configuration with default settings.
  #[must_use]
  pub const fn new() -> Self {
    Self { _reserved: () }
  }
}

impl Default for TokioTransportConfig {
  fn default() -> Self {
    Self::new()
  }
}
