//! Abstraction over gossip lifecycle.

/// Drives gossip start/stop around membership dissemination.
pub trait Gossiper: Send + Sync {
  /// Starts gossip dissemination.
  ///
  /// # Errors
  ///
  /// Returns an error if gossip dissemination fails to start.
  fn start(&mut self) -> Result<(), &'static str>;

  /// Stops gossip dissemination.
  ///
  /// # Errors
  ///
  /// Returns an error if gossip dissemination fails to stop.
  fn stop(&mut self) -> Result<(), &'static str>;
}
